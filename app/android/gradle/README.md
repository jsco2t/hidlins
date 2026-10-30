# Android dependency policy

JUnit is forbidden in the application, device tests, and resolved Gradle build
dependencies. `no-junit.init.gradle` applies before settings and plugin resolution,
including Flutter's included build. `make app-deps` installs it in the ignored,
repository-local Gradle home; Android build/runner scripts also prepare that home
before launching Gradle or Flutter. Use the Makefile workflows.

The platform device runner extends `android.app.Instrumentation` and retains all
11 security/storage cases. Five small first-party assertions replace JUnit's
assertions. Flutter's SDK integration plugin keeps its method-channel and
screenshot implementation for `flutter drive`; its separate `FlutterTestRunner`
source and AndroidX/JUnit runner dependencies are excluded from compilation and
resolution. No SDK sources are copied or modified.

## Apache Commons runtime metadata

Excluding runtime JUnit dependencies alone is insufficient: Commons parent POMs
69, 71, and 72 import JUnit BOMs while Maven metadata is being parsed. Four
already-required Apache-2.0 libraries therefore use artifact-only Maven Central
resolution with their reviewed runtime edges declared in the initialization
script. Versions, JAR bytes, and committed SHA-256 checks stay unchanged.

| Component | Required non-optional runtime dependencies |
| --- | --- |
| `commons-io:commons-io:2.16.1` | None |
| `commons-codec:commons-codec:1.17.1` | None |
| `org.apache.commons:commons-lang3:3.16.0` | None |
| `org.apache.commons:commons-compress:1.27.1` | The three components above |

This is restricted to those exact component versions. Optional, provided, and
upstream test dependencies are not runtime edges. Other dependencies continue
using their original repositories and metadata. When changing a pinned component,
review its official Maven Central POM, inherited license, and runtime dependencies
again; update the policy and regression together. Do not admit JUnit BOMs, ignore
verification errors, or disable metadata verification.

These are transitive build dependencies already required by the Android tooling;
the change adds no direct dependency, crate, or replacement library. Reimplementing
the Android tools' compression/IO libraries would add unsupported maintenance
work. The narrow metadata declarations remove the forbidden test imports while
retaining the existing library implementations.

`make android-dependency-check` resolves disposable offline Maven fixtures through
real Gradle, including settings classpaths, project classpaths, an included build
with settings-only repositories, and direct/transitive forbidden dependencies.
It asserts the complete retained runtime graph, rejects a modified JAR under
strict verification, tests assertion failure behavior without secret disclosure,
and rejects incomplete/crashed instrumentation output. APK inspection also rejects
JUnit class references in shipping and scenario-test artifacts.
