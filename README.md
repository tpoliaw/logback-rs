# Logback classes as rust structs

A straight mapping from Logback types to rust equivalents. No effort is made to
replicate logging behaviour. The purpose of the crate is to allow logging
messages sent via a ServerSocketAppender to be deserialised by a rust
application.

Only basic functionality is provided beyond this.

## Formatting messages
Deserialised messages consist of the template and the string arguments.
`LogEvents` have a `message` method that replaces placeholders in the message.

## Formatting stack traces

Exceptions/Throwables only have a list of `StackTraceElement`s so a basic
`format_trace` method is provided to simplify printing stack traces. Eventually
the deduplication of wrapped exceptions will be handled correctly as well.
