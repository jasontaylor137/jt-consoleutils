//! `${VAR}` expansion helper.
//!
//! Substitutes `${ENV_VAR}` patterns from the host process environment.
//! Undefined variables expand to empty string. A bare `$` not followed by `{`
//! is left untouched.

use std::env;

/// Expand `${ENV_VAR}` patterns from the host environment.
/// Undefined variables expand to empty string.
#[must_use]
pub fn expand_env_vars(input: &str) -> String {
   let mut result = String::with_capacity(input.len());
   let mut chars = input.chars().peekable();

   while let Some(ch) = chars.next() {
      if ch == '$' && chars.peek() == Some(&'{') {
         chars.next(); // consume '{'
         let mut var_name = String::new();
         for c in chars.by_ref() {
            if c == '}' {
               break;
            }
            var_name.push(c);
         }
         result.push_str(&env::var(&var_name).unwrap_or_default());
      } else {
         result.push(ch);
      }
   }

   result
}

#[cfg(test)]
mod tests {
   use std::sync::{Mutex, MutexGuard};

   use super::*;

   /// Process-wide lock so parallel tests don't race on the shared env.
   static ENV_LOCK: Mutex<()> = Mutex::new(());

   struct ScopedEnv {
      names: Vec<&'static str>,
      _lock: MutexGuard<'static, ()>
   }

   impl ScopedEnv {
      fn new(vars: &[(&'static str, &str)]) -> Self {
         let lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
         for (name, value) in vars {
            unsafe { env::set_var(name, value) };
         }
         Self { names: vars.iter().map(|(k, _)| *k).collect(), _lock: lock }
      }
   }

   impl Drop for ScopedEnv {
      fn drop(&mut self) {
         for name in &self.names {
            unsafe { env::remove_var(name) };
         }
      }
   }

   #[test]
   fn expand_single_var() {
      let _env = ScopedEnv::new(&[("JT_ENVVARS_TEST_TOKEN", "abc123")]);
      assert_eq!(expand_env_vars("token=${JT_ENVVARS_TEST_TOKEN}"), "token=abc123");
   }

   #[test]
   fn expand_multiple_vars() {
      let _env = ScopedEnv::new(&[("JT_ENVVARS_TEST_A", "hello"), ("JT_ENVVARS_TEST_B", "world")]);
      assert_eq!(expand_env_vars("${JT_ENVVARS_TEST_A} ${JT_ENVVARS_TEST_B}"), "hello world");
   }

   #[test]
   fn expand_undefined_var_to_empty() {
      assert_eq!(expand_env_vars("prefix-${JT_ENVVARS_UNDEFINED_12345}-suffix"), "prefix--suffix");
   }

   #[test]
   fn expand_no_vars_passes_through() {
      assert_eq!(expand_env_vars("no variables here"), "no variables here");
   }

   #[test]
   fn expand_dollar_without_brace_passes_through() {
      assert_eq!(expand_env_vars("cost is $5"), "cost is $5");
   }

   #[test]
   fn expand_empty_input() {
      assert_eq!(expand_env_vars(""), "");
   }
}
