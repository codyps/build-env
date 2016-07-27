use std::env;
use std::ffi::{OsStr,OsString};
use std::borrow::ToOwned;
use std::{error,fmt,any};

/**
 * Allow retrieval of values pretaining to a `build` process that may be related to the `target`
 * and/or `host` triple.
 *
 */
#[derive(Debug,Clone)]
pub struct BuildEnv {
    /*
     * restricted to String due to our use of String::replace
     */
    target: String,
    host: String,
}

/// If variable retrieval fails, it will be for one of these reasons
#[derive(Debug,Clone)]
pub enum VarErrorKind {
    NotFound,
    NotString(OsString),
}

/// Describes a variable retrieval failure
#[derive(Debug,Clone)]
pub struct VarError<K: AsRef<OsStr>> {
    key: K,
    kind: VarErrorKind
}

impl<K: AsRef<OsStr>> fmt::Display for VarError<K> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self.kind {
            VarErrorKind::NotFound => write!(fmt, "Variable {:?} was not found", self.key.as_ref()),
            VarErrorKind::NotString(ref x) => write!(fmt, "Variable {:?} was found, but is not utf-8: {:?}",
                                                     self.key.as_ref(), x)
        }
    }
}

impl<K: AsRef<OsStr> + fmt::Debug + any::Any> error::Error for VarError<K> {
    fn description(&self) -> &str {
        match self.kind {
            VarErrorKind::NotFound => "not found",
            VarErrorKind::NotString(_) => "found but not utf-8",
        }
    }
}

impl BuildEnv {
    /**
     * Use environment variables (such as those set by cargo) to determine values for `target` and
     * `host` via the environment variables `TARGET` and `HOST`.
     */
    pub fn from_env() -> Result<BuildEnv, env::VarError> {
        let target = try!(env::var("TARGET"));
        let host = try!(env::var("HOST"));

        Ok(BuildEnv {
            target: target,
            host: host,
        })
    }

    /**
     * Construct a BuildEnv where the host and target _may_ be different.
     */
    pub fn new_cross(host: String, target: String) -> BuildEnv {
        BuildEnv {
            host: host,
            target: target
        }
    }

    /**
     * Construct a BuildEnv where target and host are the same.
     */
    pub fn new(trip: String) -> BuildEnv {
        BuildEnv {
            host: trip.clone(),
            target: trip,
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

    /**
     * Query the environment for a value, trying the most specific first, before querying more
     * general variables.
     * 
     * 1. `<var>_<target>` - for example, `CC_x86_64-unknown-linux-gnu`
     * 2. `<var>_<target_with_underscores>` - for example, `CC_x86_64_unknown_linux_gnu`
     * 3. `<build-kind>_<var>` - for example, `HOST_CC` or `TARGET_CFLAGS`
     * 4. `<var>` - a plain `CC`, `AR` as above.
     */
    pub fn var<K: AsRef<OsStr>>(&self, var_base: K) -> Option<OsString>
    {
        /* try the most specific item to the least specific item */
        let target = self.target();
        let host = self.host();
        let kind = if host == target {"HOST"} else {"TARGET"};
        let target_u = target.replace("-", "_");
        let mut a : OsString = var_base.as_ref().to_owned();
        a.push("_");

        let mut b = a.clone();

        a.push(target);
        b.push(target_u);

        let mut c : OsString = AsRef::<OsStr>::as_ref(kind).to_owned();
        c.push("_");
        c.push(&var_base);

        env::var_os(&a)
            .or_else(|| env::var_os(&b))
            .or_else(|| env::var_os(&c))
            .or_else(|| env::var_os(var_base))
    }

    /**
     * The same as Self::var(), but converts the return to an OsString and provides a useful error
     * message
     */
    pub fn var_str<K: AsRef<OsStr> + fmt::Debug + any::Any>(&self, var_base: K) -> Result<String, VarError<K>>
    {
        match self.var(&var_base) {
            Some(v) => v.into_string().map_err(|o| VarError { key: var_base, kind: VarErrorKind::NotString(o)}),
            None => Err(VarError { key: var_base, kind: VarErrorKind::NotFound }),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use super::BuildEnv;
    fn clear(trip: &str, var: &[&str]) {
        for v in var {
            env::remove_var(&format!("HOST_{}", v));
            env::remove_var(&format!("TARGET_{}", v));
            env::remove_var(&format!("{}_{}", trip, v));
            env::remove_var(&format!("{}_{}", trip.replace("-","_"), v));
            env::remove_var(v);
        }
    }

    #[test]
    fn most_general() {
        let t = "this-is-a-target";
        let cc = "a-cc-value";
        clear(t, &["CC"]);
        env::set_var("CC", cc);

        let b = BuildEnv::new(t.to_owned());

        assert_eq!(b.var_str("CC").unwrap(), cc);
    }

    #[test]
    fn exact_target() {
        let t = "this-is-a-target";
        let cc = "a-cc-value";
        clear(t, &["CC"]);
        env::set_var(format!("CC_{}", t), cc);

        let b = BuildEnv::new(t.to_owned());

        assert_eq!(b.var_str("CC").unwrap(), cc);
    }
}
