use std::borrow::ToOwned;
use std::collections::BTreeSet;
use std::env;
use std::ffi::{OsStr, OsString};
use std::{any, error, fmt};

/**
 * Allow retrieval of values pretaining to a `build` process that may be related to the `target`
 * and/or `host` triple.
 *
 */
#[derive(Debug, Clone)]
pub struct BuildEnv {
    /*
     * restricted to String due to our use of String::replace
     */
    target: String,
    host: String,

    // env vars accessed. note that we use a BTreeSet to get deterministic ordering
    used_env_vars: BTreeSet<OsString>,
}

/// If variable retrieval fails, it will be for one of these reasons
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VarErrorKind {
    NotString(OsString),
    RequiredEnvMissing(env::VarError),
}

/// Describes a variable retrieval failure
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VarError<K: AsRef<OsStr>> {
    key: K,
    kind: VarErrorKind,
}

impl<K: AsRef<OsStr>> fmt::Display for VarError<K> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self.kind {
            VarErrorKind::NotString(ref x) => write!(
                fmt,
                "Variable {:?} was found, but is not utf-8: {:?}",
                self.key.as_ref(),
                x
            ),
            VarErrorKind::RequiredEnvMissing(ref x) => write!(
                fmt,
                "Variable {:?} is required, but retrival failed: {}",
                self.key.as_ref(),
                x
            )
        }
    }
}

impl<K: AsRef<OsStr> + fmt::Debug + any::Any> error::Error for VarError<K> {
    fn description(&self) -> &str {
        match self.kind {
            VarErrorKind::NotString(_) => "found but not utf-8",
            VarErrorKind::RequiredEnvMissing(_) => "other required env var missing",
        }
    }
}

fn required_env_var(key: &str) -> Result<String, VarError<String>> {
    env::var(key)
        .map_err(|e| VarError {
            key: key.to_owned(),
            kind: VarErrorKind::RequiredEnvMissing(e)
        })
}

impl BuildEnv {
    /**
     * Use environment variables (such as those set by cargo) to determine values for `target` and
     * `host` via the environment variables `TARGET` and `HOST`.
     */
    pub fn from_env() -> Result<BuildEnv, VarError<String>> {
        // NOTE: we don't consider these env vars "used" because cargo already will call build
        // scripts again if they change
        let target = required_env_var("TARGET")?;
        let host = required_env_var("HOST")?;

        Ok(BuildEnv {
            target,
            host,
            used_env_vars: Default::default(),
        })
    }

    /**
     * Construct a BuildEnv where the host and target _may_ be different.
     */
    pub fn new_cross(host: String, target: String) -> BuildEnv {
        BuildEnv {
            host,
            target,
            used_env_vars: Default::default(),
        }
    }

    /**
     * Construct a BuildEnv where target and host are the same.
     */
    pub fn new(trip: String) -> BuildEnv {
        BuildEnv {
            host: trip.clone(),
            target: trip,
            used_env_vars: Default::default(),
        }
    }

    /**
     * The target we're supplying values for
     */
    pub fn target(&self) -> &str {
        &self.target
    }

    /**
     * The host we're supplying values for
     */
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Get the env vars that have been used by build-env queries so far
    ///
    pub fn used_env_vars(&self) -> impl Iterator<Item = &OsString> {
        self.used_env_vars.iter()
    }

    /// Print the used environment variables in the form interpreted by cargo: `cargo:rerun-if-env-changed=FOO`
    pub fn cargo_print_used_env_vars(&self) {
        for used in self.used_env_vars() {
            // NOTE: complains loudly if we use a env-var we can't track because it isn't utf-8
            println!("cargo:rerun-if-env-changed={}", used.to_str().unwrap());
        }
    }

    pub fn mark_used(&mut self, var: OsString) {
        println!(
            "cargo:rerun-if-env-changed={}",
            var.to_str().expect("tried to examine non-utf-8 variable")
        );
        self.used_env_vars.insert(var);
    }

    fn env_one(&mut self, var: OsString) -> Option<OsString> {
        let v = env::var_os(&var);
        self.mark_used(var);
        v
    }

    /// Query the environment for a value, trying the most specific first, before querying more
    /// general variables.
    ///
    /// 1. `<var>_<target>` - for example, `CC_x86_64-unknown-linux-gnu`
    /// 2. `<var>_<target_with_underscores>` - for example, `CC_x86_64_unknown_linux_gnu`
    /// 3. `<build-kind>_<var>` - for example, `HOST_CC` or `TARGET_CFLAGS`
    /// 4. `<var>` - a plain `CC`, `AR` as above.
    pub fn var<K: AsRef<OsStr>>(&mut self, var_base: K) -> Option<OsString> {
        /* try the most specific item to the least specific item */
        let target = self.target();
        let host = self.host();
        let kind = if host == target { "HOST" } else { "TARGET" };
        let target_u = target.replace("-", "_");
        let mut a: OsString = var_base.as_ref().to_owned();
        a.push("_");

        let mut b = a.clone();

        a.push(target);
        b.push(target_u);

        let mut c: OsString = AsRef::<OsStr>::as_ref(kind).to_owned();
        c.push("_");
        c.push(&var_base);

        self.env_one(a)
            .or_else(|| self.env_one(b))
            .or_else(|| self.env_one(c))
            .or_else(|| self.env_one(var_base.as_ref().to_owned()))
    }

    /// The same as [`var()`], but converts the return to an OsString and provides a useful error
    /// message
    pub fn var_str<K: AsRef<OsStr> + fmt::Debug + any::Any>(
        &mut self,
        var_base: K,
    ) -> Option<Result<String, VarError<K>>> {
        match self.var(&var_base) {
            Some(v) => Some(v.into_string().map_err(|o| VarError {
                key: var_base,
                kind: VarErrorKind::NotString(o),
            })),
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BuildEnv;
    use std::env;

    fn clear(trip: &str, var: &[&str]) {
        for v in var {
            env::remove_var(&format!("HOST_{}", v));
            env::remove_var(&format!("TARGET_{}", v));
            env::remove_var(&format!("{}_{}", v, trip));
            env::remove_var(&format!("{}_{}", v, trip.replace("-", "_")));
            env::remove_var(v);
        }
    }

    fn most_general() {
        let t = "this-is-a-target";
        let cc = "a-cc-value";
        clear(t, &["CC"]);
        env::set_var("CC", cc);

        let mut b = BuildEnv::new(t.to_owned());

        assert_eq!(b.var_str("CC"), Some(Ok(cc.to_owned())));
        let used_env_vars: Vec<_> = b.used_env_vars().collect();
        assert_eq!(
            &used_env_vars[..],
            [
                "CC",
                "CC_this-is-a-target",
                "CC_this_is_a_target",
                "HOST_CC"
            ]
        );
        clear(t, &["CC"]);
    }

    fn exact_target() {
        let t = "this-is-a-target";
        let cc = "a-cc-value";
        clear(t, &["CC"]);

        env::set_var("CC", "notThis");
        env::set_var("HOST_CC", "not-this");
        env::set_var(format!("CC_{}", t), cc);

        let mut b = BuildEnv::new(t.to_owned());

        assert_eq!(b.var_str("CC"), Some(Ok(cc.to_owned())));
        let used_env_vars: Vec<_> = b.used_env_vars().collect();
        assert_eq!(&used_env_vars[..], ["CC_this-is-a-target"]);
        clear(t, &["CC"]);
    }

    fn underscore_target() {
        let t = "this-is-a-target";
        let cc = "a-cc-value";
        clear(t, &["CC"]);

        env::set_var("CC", "notThis");
        env::set_var("HOST_CC", "not-this");
        env::set_var("CC_this_is_a_target", cc);

        let mut b = BuildEnv::new(t.to_owned());

        assert_eq!(b.var_str("CC"), Some(Ok(cc.to_owned())));
        let used_env_vars: Vec<_> = b.used_env_vars().collect();
        assert_eq!(
            &used_env_vars[..],
            ["CC_this-is-a-target", "CC_this_is_a_target"]
        );
        clear(t, &["CC"]);
    }

    fn v_host() {
        let t = "this-is-a-target";
        let cc = "a-cc-value";
        clear(t, &["CC"]);

        env::set_var("CC", "not-this-value");
        env::set_var("HOST_CC", cc);

        let mut b = BuildEnv::new(t.to_owned());

        assert_eq!(b.var_str("CC"), Some(Ok(cc.to_owned())));
        let used_env_vars: Vec<_> = b.used_env_vars().collect();
        assert_eq!(
            &used_env_vars[..],
            ["CC_this-is-a-target", "CC_this_is_a_target", "HOST_CC"]
        );
        clear(t, &["CC"]);
    }

    fn v_target() {
        let t = "this-is-a-target";
        let t2 = "some-target";
        let cc = "a-cc-value";
        clear(t, &["CC"]);
        clear(t2, &["CC"]);

        env::set_var("CC", "not-this-value");
        env::set_var("HOST_CC", "not this!");
        env::set_var("TARGET_CC", cc);
        env::set_var(format!("CC_{}", t), "not this either");

        let mut b = BuildEnv::new_cross(t.to_owned(), t2.to_owned());

        assert_eq!(b.var_str("CC"), Some(Ok(cc.to_owned())));
        let used_env_vars: Vec<_> = b.used_env_vars().collect();
        assert_eq!(
            &used_env_vars[..],
            ["CC_some-target", "CC_some_target", "TARGET_CC"]
        );
        clear(t, &["CC"]);
    }

    /* tests are only run in seperate threads, and seperate threads share environment between them.
     * This causes our tests to fail when run concurrently.
     *
     * Workaround this for now by explicitly running them sequentially. Correct fix is probably to
     * provide a "virtual" environment of sorts.
     */
    #[test]
    fn all() {
        most_general();
        exact_target();
        underscore_target();
        v_host();
        v_target();
    }
}
