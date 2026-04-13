//! Utilities for subcommand argument parsing.

/// Convert a slice of `String` args into a [`pico_args::Arguments`] for
/// optional flag extraction within subcommand parsers.
pub fn to_pargs(args: &[String]) -> pico_args::Arguments {
   pico_args::Arguments::from_vec(args.iter().map(std::ffi::OsString::from).collect())
}
