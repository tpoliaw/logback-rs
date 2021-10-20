use jaded::FromJava;
use std::{
    borrow::Cow,
    collections::HashMap,
    fmt::{Display, Error as FmtError, Formatter},
    str::FromStr,
};

pub enum Error {
    ValueError(String),
}

#[derive(Debug, FromJava)]
#[jaded(rename)]
pub struct LogEvent {
    #[jaded(field = "message")]
    template: String,
    thread_name: String,
    pub logger_name: String,
    #[jaded(field = "loggerContextVO")]
    pub context: LogContext,
    #[jaded(extract(converters::read_i32), from = "i32")]
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

enum FormatState {
    Escape, // We've seen an escape char
    Started, // We've seen an opening {
    Normal, // Nothing interesting - normal text
}

fn format<'a>(template: &'a str, args: &[String]) -> Cow<'a, str> {
    if template.contains("{}") {
        use FormatState::*;
        let mut args = args.into_iter();
        let mut message = String::new();
        let mut state = Normal;
        for c in template.chars() {
            match (&state, c) {
                (Normal, '\\') => state = Escape,
                (Normal, '{') => state = Started,
                (Normal, c) => message.push(c),
                (Escape, '\\') => {
                    message.push_str("\\\\");
                    state = Normal;
                },
                (Escape, '{') => {
                    message.push('{');
                    state = Normal;
                },
                (Escape, c) => {
                    message.push('\\');
                    message.push(c);
                    state = Normal;
                },
                (Started, '}') => {
                    message.push_str(match args.next() {
                        Some(a) => &a,
                        None => "{}",
                    });
                    state = Normal;
                },
                (Started, '{') => message.push('{'),
                (Started, '\\') => {
                    message.push('{');
                    state = Escape;
                },
                (Started, c) => {
                    message.push('{');
                    message.push(c);
                    state = Normal;
                }
            }
        }
        Cow::Owned(message)
    } else {
        Cow::Borrowed(template)
    }
}

impl LogEvent {
    pub fn message(&self) -> Cow<str> {
        format(&self.template,  &self.arguments)
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
    #[jaded(field = "propertyMap")]
    pub properties: PropertyMap,
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
    markers: Vec<Marker>
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
            _ => {
                return Err(Error::ValueError(
                    format!("Unknown log level: {}", s).into(),
                ))
            }
        })
    }
}

#[derive(Debug, FromJava)]
#[jaded(rename)]
pub struct PropertyMap {
    threshold: i32,
    load_factor: f32,
    #[jaded(extract(converters::read_map))]
    pub values: HashMap<String, String>,
}

mod converters {
    use jaded::{AnnotationIter, ConversionResult, FromJava};
    use std::{collections::HashMap, hash::Hash};
    pub fn read_i32(anno: &mut AnnotationIter) -> ConversionResult<i32> {
        Ok(anno.read_i32()?)
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
        // println!("Reading ints");
        let mut map = HashMap::new();
        let _ = anno.read_i32()?; // read and discard number of buckets
        let count = anno.read_i32()?;
        // println!("Reading {} entries", count);
        for _ in 0..count {
            map.insert(anno.read_object_as()?, anno.read_object_as()?);
        }
        Ok(map)
    }

    #[derive(Debug, FromJava)]
    pub enum Map {
        #[jaded(class="java.util.Collections$EmptyMap")]
        Empty,
        #[jaded(class="java.util.HashMap")]
        HashMap(#[jaded(extract(read_map))] HashMap<String, String>),
        #[jaded(class="java.util.Collections$SynchronizedMap")]
        Sync(#[jaded(field="m", from = "Map")] HashMap<String, String>),
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
