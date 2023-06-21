use jaded::FromJava;
use std::{
    borrow::Cow,
    collections::HashMap,
    fmt::{Display, Error as FmtError, Formatter},
    str::FromStr,
};
use time::{OffsetDateTime, PrimitiveDateTime};

pub enum Error {
    UnknownLogLevel(String),
}

impl Display for Error {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::UnknownLogLevel(msg) => write!(fmt, "Unrecognised log level: {msg:?}"),
        }
    }
}

#[derive(Debug, FromJava)]
#[jaded(rename)]
pub struct LogEvent {
    #[jaded(field = "message")]
    template: String,
    thread_name: String,
    pub logger_name: Source,
    #[jaded(field = "loggerContextVO")]
    pub context: LogContext,
    #[jaded(extract(converters::read_i32))]
    pub level: LogLevel,
    #[jaded(extract(converters::read_list))]
    arguments: Vec<String>,
    #[jaded(field = "throwableProxy")]
    pub throwable: Option<Throwable>,
    #[jaded(field = "callerDataArray")]
    stacktrace: Option<Vec<StackFrame>>,
    pub marker: Option<Marker>,
    time_stamp: i64,
    #[jaded(field = "mdcPropertyMap", from = "converters::Map")]
    pub mdc: HashMap<String, String>,
}

#[derive(Debug, FromJava)]
#[jaded(from = "String")]
pub struct Source(String);

impl From<String> for Source {
    fn from(src: String) -> Self {
        Self(src)
    }
}

impl std::fmt::Display for Source {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        match fmt.precision() {
            Some(w) => write!(fmt, "{}", self.reduced(w)),
            None => self.0.fmt(fmt),
        }
    }
}

impl Source {
    fn reduced(&self, target: usize) -> Cow<str> {
        if self.0.len() <= target {
            Cow::Borrowed(&self.0)
        } else {
            let len = self.0.len();
            let mut cut = 0;
            let mut parts = self.0.split('.').collect::<Vec<_>>();
            let mut res = vec![];
            let class = parts.pop().unwrap(); // must be present as self.0 has length
            for part in parts.into_iter() {
                if len - cut > target {
                    res.push(part.split_at(1).0);
                    cut += part.len() - 1;
                } else {
                    res.push(part);
                }
            }
            res.push(class);
            Cow::Owned(res.join("."))
        }
    }
}

#[test]
fn test_source_reduction() {
    let s = Source("uk.ac.diamond.daq.persistence.jythonshelf".into());
    assert_eq!(s.reduced(20), "u.a.d.d.p.jythonshelf");
    assert_eq!(s.reduced(30), "u.a.d.d.p.jythonshelf");
    assert_eq!(s.reduced(32), "u.a.d.d.persistence.jythonshelf");
    assert_eq!(s.reduced(33), "u.a.d.daq.persistence.jythonshelf");
    assert_eq!(s.reduced(39), "u.a.diamond.daq.persistence.jythonshelf");
    assert_eq!(s.reduced(40), "u.ac.diamond.daq.persistence.jythonshelf");
    assert_eq!(s.reduced(41), "uk.ac.diamond.daq.persistence.jythonshelf");
    assert_eq!(s.reduced(50), "uk.ac.diamond.daq.persistence.jythonshelf");

    let s = Source("gda.device.scannable.ScannableMotor".into());
    assert_eq!(s.reduced(30), "g.d.scannable.ScannableMotor");

    let s = Source("gdascripts.scan.process.ScanDataProcessorResult.ScanDataProcessorResult".into());
    assert_eq!(s.reduced(30), "g.s.p.S.ScanDataProcessorResult");
}

fn format<'a>(template: &'a str, args: &[String]) -> Cow<'a, str> {
    const ANCHOR: &str = "{}";
    const ESC: char = '\\';
    const OPEN: char = '{';
    const CLOSE: char = '}';
    const NULL_STRING: &str = "NULL_ARGUMENT_ARRAY_ELEMENT";
    const NULL: &str = "null";
    if !args.is_empty() && template.contains("{}") {
        let mut message = String::new();
        let mut args = args.iter();
        let mut chars = template.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                ESC => match chars.next() {
                    Some(OPEN) if chars.peek() == Some(&CLOSE) => message.push(OPEN),
                    Some(c) => {
                        // If the escape isn't escaping a complete {},
                        // include the escape in the message
                        message.push(ESC);
                        message.push(c);
                    }
                    None => message.push(ESC),
                },
                OPEN => match chars.peek() {
                    Some(&CLOSE) => {
                        let _ = chars.next(); // drop closing char
                        match args.next().map(String::as_str) {
                            Some(NULL_STRING) => message.push_str(NULL),
                            Some(a) => message.push_str(a),
                            None => {
                                message.push_str(ANCHOR);
                                chars.for_each(|c| message.push(c));
                                break;
                            }
                        }
                    }
                    _ => message.push(OPEN),
                },
                c => message.push(c),
            }
        }
        Cow::Owned(message)
    } else {
        Cow::Borrowed(template)
    }
}

impl LogEvent {
    pub fn message(&self) -> Cow<str> {
        format(&self.template, &self.arguments)
    }
    pub fn time(&self) -> OffsetDateTime {
        let nanos = 1_000_000 * self.time_stamp as i128;
        OffsetDateTime::from_unix_timestamp_nanos(nanos).unwrap()
    }
    pub fn stack(&self) -> String {
        match &self.throwable {
            Some(t) => format!("\n{}{}", t.class_name, t.trace()),
            None => format!(""),
        }
    }
}

#[derive(Debug, FromJava)]
#[jaded(rename)]
pub struct LogContext {
    birth_time: i64,
    name: String,
    #[jaded(field = "propertyMap", from = "PropertyMap")]
    pub properties: HashMap<String, String>,
}

#[derive(Debug, FromJava)]
pub struct Throwable {
    #[jaded(field = "className")]
    class_name: String,
    message: Option<String>,
    #[jaded(field = "commonFramesCount")]
    common_frames: i32,
    cause: Option<Box<Throwable>>,
    suppressed: Vec<Throwable>,
    #[jaded(field = "stackTraceElementProxyArray")]
    stack_trace: Vec<StackTraceElement>,
}

impl Throwable {
    fn trace(&self) -> String {
        self.stack_trace
            .iter()
            .map(|ste| format!("{}", ste))
            .collect::<Vec<_>>()
            .join("\n     at ")
    }
}

#[derive(Debug, FromJava)]
#[jaded(rename)]
pub struct StackFrame {
    declaring_class: Option<String>,
    #[jaded(field = "lineNumber")]
    line: i32,
    class_loader_name: Option<String>,
    method_name: Option<String>,
    module_name: Option<String>,
    format: u8,
    module_version: Option<String>,
    file_name: Option<String>,
}

#[derive(Debug, FromJava)]
pub struct StackTraceElement {
    ste: StackFrame,
    cpd: Option<ClassPackagingData>,
}

impl Display for StackTraceElement {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            fmt,
            "{}.{}({}:{})",
            &self.ste.declaring_class.as_ref().unwrap(),
            &self.ste.method_name.as_ref().unwrap(),
            &self.ste.file_name.as_ref().unwrap(),
            &self.ste.line
        )
    }
}

#[derive(Debug, FromJava)]
pub struct ClassPackagingData {
    code_location: String,
    version: String,
    exact: bool,
}

#[derive(Debug, FromJava)]
pub struct Marker {
    name: String,
    #[jaded(field = "referenceList", from = "Markers")]
    references: Vec<Marker>,
}

#[derive(Debug, FromJava)]
pub struct Markers {
    #[jaded(extract(converters::read_list))]
    markers: Vec<Marker>,
}

impl From<Markers> for Vec<Marker> {
    fn from(markers: Markers) -> Self {
        markers.markers
    }
}

#[derive(Debug, PartialEq, PartialOrd)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Unknown,
}

impl LogLevel {
    pub fn name(&self) -> &'static str {
        use LogLevel::*;
        match self {
            Trace => "TRACE",
            Debug => "DEBUG",
            Info => "INFO",
            Warn => "WARN",
            Error => "ERROR",
            Unknown => "UNKNOWN",
        }
    }
}

impl Display for LogLevel {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), FmtError> {
        fmt.write_str(self.name())
    }
}

impl From<i32> for LogLevel {
    fn from(value: i32) -> Self {
        match value {
            5_000 => Self::Trace,
            10_000 => Self::Debug,
            20_000 => Self::Info,
            30_000 => Self::Warn,
            40_000 => Self::Error,
            _ => Self::Unknown,
        }
    }
}

impl FromStr for LogLevel {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "t" | "trace" => Self::Trace,
            "d" | "debug" => Self::Debug,
            "i" | "info" => Self::Info,
            "w" | "warn" => Self::Warn,
            "e" | "error" => Self::Error,
            _ => return Err(Error::UnknownLogLevel(s.into())),
        })
    }
}

#[derive(Debug, FromJava)]
struct PropertyMap {
    #[jaded(extract(converters::read_map))]
    pub values: HashMap<String, String>,
}

impl From<PropertyMap> for HashMap<String, String> {
    fn from(value: PropertyMap) -> Self {
        value.values
    }
}

mod converters {
    use jaded::{AnnotationIter, ConversionResult, FromJava};
    use std::{collections::HashMap, hash::Hash};
    pub fn read_i32(anno: &mut AnnotationIter) -> ConversionResult<i32> {
        anno.read_i32()
    }
    pub fn read_list<T>(anno: &mut AnnotationIter) -> ConversionResult<Vec<T>>
    where
        T: FromJava,
    {
        (0..anno.read_i32()?)
            .map(|_| anno.read_object_as())
            .collect()
    }
    pub fn read_map<T, U>(anno: &mut AnnotationIter) -> ConversionResult<HashMap<T, U>>
    where
        T: FromJava + Eq + Hash,
        U: FromJava,
    {
        let mut map = HashMap::new();
        let _ = anno.read_i32()?; // read and discard number of buckets
        let count = anno.read_i32()?;
        for _ in 0..count {
            map.insert(anno.read_object_as()?, anno.read_object_as()?);
        }
        Ok(map)
    }

    #[derive(Debug, FromJava)]
    pub enum Map {
        #[jaded(class = "java.util.Collections$EmptyMap")]
        Empty,
        #[jaded(class = "java.util.HashMap")]
        HashMap(#[jaded(extract(read_map))] HashMap<String, String>),
        #[jaded(class = "java.util.Collections$SynchronizedMap")]
        Sync(#[jaded(field = "m", from = "Map")] HashMap<String, String>),
    }
    impl From<Map> for HashMap<String, String> {
        fn from(map: Map) -> HashMap<String, String> {
            match map {
                Map::Empty => HashMap::with_capacity(0),
                Map::HashMap(v) => v,
                Map::Sync(m) => m,
            }
        }
    }
}

#[test]
fn test_format() {
    assert_eq!(format("no anchors", &[]), Cow::Borrowed("no anchors"));
    assert_eq!(
        format("single {} anchor", &["central".into()]),
        Cow::Owned::<str>("single central anchor".into())
    );
    assert_eq!(
        format("unused arg", &["foo".into()]),
        Cow::Borrowed("unused arg")
    );
    assert_eq!(
        format("unused {} anchor", &[]),
        Cow::Borrowed("unused {} anchor")
    );
    assert_eq!(
        format(r"escaped escape \\{}", &["foo".into()]),
        Cow::Owned::<str>(r"escaped escape \\foo".into())
    );
    assert_eq!(
        format(r"Partially escaped \{ anchor", &[]),
        Cow::Borrowed(r"Partially escaped \{ anchor".into())
    );
    assert_eq!(
        format(r"Partially escaped \{ anchor with {}", &["arg".into()]),
        Cow::Owned::<str>(r"Partially escaped \{ anchor with arg".into())
    );
    assert_eq!(
        format(r"End with {} escape\", &["final".into()]),
        Cow::Owned::<str>(r"End with final escape\".into())
    );
    assert_eq!(
        format("Too {} arguments {}", &["few".into()]),
        Cow::Borrowed("Too few arguments {}")
    );
    assert_eq!(
        format("Too {} arguments", &["many".into(), "ignored".into()]),
        Cow::Borrowed("Too many arguments")
    );
    assert_eq!(
        format("Not {} an {anchor}", &["really".into()]),
        Cow::Owned::<str>("Not really an {anchor}".into())
    );
}
