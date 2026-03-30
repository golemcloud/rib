# rib-core

Core library for the Rib language: parser, type inference, compiler, and interpreter.

## Rib compilation profile

Rib type inference has multiple phases. Each phase will contribute to the total time taken for compiling a rib script.
There are a few hotspots now which could be fixed. To understand which phase is taking more time, run any test setting the RIB_PROFILE to 1

![img.png](images/img.png)

**Set the environment variable** for the single command that runs the compiler (prefix the command; no code changes needed):

   ```bash
   RIB_PROFILE=1 cargo test -p rib-core --test rib_regression -- --nocapture
   ```
