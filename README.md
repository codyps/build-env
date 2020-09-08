# build-env

Get information about the build from environment variables

```
let mut be = build_env::BuildEnv::from_env().unwrap();
let _: Option<OsString> = be.var_os("CC");
```

Prints all used env vars in cargo dependency format:

```
cargo:rerun-if-env-changed=CC_target-triple
cargo:rerun-if-env-changed=CC_target_triple
cargo:rerun-if-env-changed=HOST_CC
cargo:rerun-if-env-changed=CC
```


[![Documentation](https://img.shields.io/badge/documentation-latest-brightgreen.svg?style=flat)](https://docs.rs/docs/build-env)
[![Crates.io](https://img.shields.io/crates/v/build-env.svg?maxAge=2592000)](https://crates.io/crates/build-env)
[![Travis](https://img.shields.io/travis/jmesmon/build-env.svg?maxAge=2592000)](https://travis-ci.org/jmesmon/build-env)
