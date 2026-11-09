import 'dart:convert';
import 'dart:io';

import 'package:build_tool/src/builder.dart';
import 'package:build_tool/src/cargo.dart';
import 'package:build_tool/src/options.dart';
import 'package:build_tool/src/rustup.dart';
import 'package:build_tool/src/target.dart';
import 'package:build_tool/src/util.dart';

void require(bool condition, String message) {
  if (!condition) throw StateError(message);
}

Future<void> exercise(
  Directory repository,
  Directory fixture,
  Target target,
  BuildConfiguration configuration, {
  String inventory = 'both',
  bool missingTarget = false,
  bool missingNdk = false,
  bool zig = false,
  bool missingZig = false,
  String? badPin,
  bool missingPin = false,
  String? missingAndroidSetting,
  String? expectedError,
}) async {
  final pin = RegExp(r'channel\s*=\s*"(\d+\.\d+\.\d+)"')
      .firstMatch(
        File('${repository.path}/rust-toolchain.toml').readAsStringSync(),
      )!
      .group(1)!;
  var manifest = Directory('${repository.path}/crates/hidlins-api');
  if (missingPin || badPin != null) {
    manifest = Directory('${fixture.path}/workspace/crate')
      ..createSync(recursive: true);
    File('${manifest.path}/Cargo.toml')
        .writeAsStringSync('[package]\nname="hidlins-api"\nversion="0.1.0"\n');
    if (badPin != null)
      File('${manifest.parent.path}/rust-toolchain.toml')
          .writeAsStringSync(badPin);
  }
  final sdk = Directory('${fixture.path}/sdk');
  const ndkVersion = '29.0.14524491';
  final ndk = Directory('${sdk.path}/ndk/$ndkVersion')
    ..createSync(recursive: true);
  if (!missingNdk)
    File('${ndk.path}/package.xml').writeAsStringSync('<fixture/>');
  final host = Platform.isMacOS ? 'darwin-x86_64' : 'linux-x86_64';
  final bin = Directory('${ndk.path}/toolchains/llvm/prebuilt/$host/bin')
    ..createSync(recursive: true);
  File('${bin.path}/llvm-ar').writeAsStringSync('');
  final commands = <TestRunCommandArgs>[];
  testRunCommandOverride = (command) {
    commands.add(command);
    if (command.executable == 'python3' &&
        command.arguments.length > 2 &&
        command.arguments[0].endsWith('vendor_cache.py') &&
        command.arguments[1] == 'describe') {
      final selected = Directory(
        command.arguments[command.arguments.indexOf('--target-dir') + 1],
      ).absolute;
      final name = selected.path.split(Platform.pathSeparator).last;
      return TestRunCommandResult(
        stdout: jsonEncode({
          'target_directory': selected.path,
          'build_directory': selected.path,
          'lock_paths': ['${selected.parent.path}/.$name.hidlins-vendor.lock'],
        }),
      );
    }

    final args = command.arguments;
    if (args.length >= 2 && args[0] == 'toolchain' && args[1] == 'list') {
      return TestRunCommandResult(
        stdout: [
          if (inventory == 'both' || inventory == 'pinned')
            '$pin-x86_64-unknown-linux-gnu (active)',
          if (inventory == 'both' || inventory == 'stable')
            'stable-x86_64-unknown-linux-gnu (default)',
        ].join('\n'),
      );
    }
    if (args.length >= 2 && args[0] == 'target' && args[1] == 'list') {
      return TestRunCommandResult(
        stdout: missingTarget ? '' : '${target.rust}\n',
      );
    }
    if (args.contains('zigbuild') && args.contains('--version')) {
      return TestRunCommandResult(
        exitCode: missingZig ? 1 : 0,
        stdout: 'fixture-version',
      );
    }
    return TestRunCommandResult();
  };
  try {
    final builder = RustBuilder(
      target: target,
      environment: BuildEnvironment(
        configuration: configuration,
        crateOptions: CargokitCrateOptions(),
        targetTempDir: '${fixture.path}/artifacts',
        manifestDir: manifest.path,
        crateInfo: CrateInfo.load(manifest.path),
        isAndroid: target.android != null,
        androidSdkPath: missingAndroidSetting == 'sdk' ? null : sdk.path,
        androidNdkVersion: missingAndroidSetting == 'ndkVersion'
            ? null
            : ndkVersion,
        androidMinSdkVersion: missingAndroidSetting == 'minSdk' ? null : 29,
        javaHome: '${fixture.path}/java',
        glibcVersion: zig ? '2.17' : null,
      ),
    );
    BuildException? failure;
    try {
      if (missingAndroidSetting == null) builder.prepare(Rustup());
      await builder.build();
    } on BuildException catch (error) {
      failure = error;
    }
    final provisioning = commands.where(
      (command) => command.arguments.any(
        (arg) => ['install', 'add', '--install'].contains(arg),
      ),
    );
    require(provisioning.isEmpty, 'normal-build-provisions-tools');
    final builds = commands
        .where(
          (command) =>
              command.arguments.length >= 4 &&
              command.arguments[0] == 'run' &&
              command.arguments[2] == 'cargo' &&
              ['build', 'zigbuild'].contains(command.arguments[3]) &&
              !command.arguments.contains('--version'),
        )
        .toList();
    if (expectedError != null) {
      require(failure != null, 'missing-prerequisite-was-not-rejected');
      require(
        failure!.message.toLowerCase().contains(expectedError.toLowerCase()),
        'missing-prerequisite-diagnostic',
      );
      require(builds.isEmpty, 'build-started-with-missing-prerequisite');
    } else {
      require(failure == null, 'installed-prerequisites-were-rejected');
      require(builds.length == 1, 'expected-one-cargo-build');
      final args = builds.single.arguments;
      require(args[1] == pin, 'mutable-or-wrong-compiler-selected');
      require(
        args.contains('--offline') && args.contains('--locked'),
        'offline-lock-guards-lost',
      );
      require(
        args.contains('--release') ==
            (configuration != BuildConfiguration.debug),
        'profile-changed',
      );
      require(
        args[args.indexOf('--target') + 1].startsWith(target.rust),
        'target-changed',
      );
    }
  } finally {
    testRunCommandOverride = null;
  }
}

Future<void> main(List<String> arguments) async {
  require(arguments.length == 1, 'repository-argument-required');
  final repository = Directory(arguments.single).absolute;
  final root = Directory.systemTemp.createTempSync('hidlins-pinned-cargokit-');
  var passed = 0;
  var failed = 0;
  var ordinal = 0;
  Future<void> run(String name, Future<void> Function(Directory) body) async {
    final fixture = Directory('${root.path}/${ordinal++}')..createSync();
    try {
      await body(fixture);
      passed++;
      stdout.writeln('PASS $name');
    } on StateError catch (error) {
      failed++;
      stdout.writeln('FAIL $name ${error.message}');
    } catch (error) {
      failed++;
      stdout.writeln('FIXTURE_ERROR $name ${error.runtimeType}');
    }
  }

  final linux = Target.forFlutterName('linux-x64')!;
  final android = Target.forFlutterName('android-arm64')!;
  final targets = [
    linux,
    Target.forFlutterName('linux-arm64')!,
    android,
    Target.forFlutterName('android-x64')!,
    Target.forDarwin(platformName: 'macosx', darwinAarch: 'arm64')!,
    Target.forDarwin(platformName: 'macosx', darwinAarch: 'x86_64')!,
    Target.forDarwin(platformName: 'iphoneos', darwinAarch: 'arm64')!,
    Target.forDarwin(platformName: 'iphonesimulator', darwinAarch: 'arm64')!,
    Target.forDarwin(platformName: 'iphonesimulator', darwinAarch: 'x86_64')!,
  ];
  try {
    for (final configuration in BuildConfiguration.values) {
      for (final target in targets) {
        await run(
          '${target.rust}/${configuration.name}/pin-wins-over-stable',
          (fixture) => exercise(repository, fixture, target, configuration),
        );
      }
    }
    await run(
      'pinned-release-without-stable-alias',
      (fixture) => exercise(
        repository,
        fixture,
        linux,
        BuildConfiguration.debug,
        inventory: 'pinned',
      ),
    );
    await run(
      'stable-alias-is-not-the-exact-installed-pin',
      (fixture) => exercise(
        repository,
        fixture,
        linux,
        BuildConfiguration.debug,
        inventory: 'stable',
        expectedError: 'toolchain',
      ),
    );
    await run(
      'missing-toolchain-never-installs',
      (fixture) => exercise(
        repository,
        fixture,
        linux,
        BuildConfiguration.debug,
        inventory: 'none',
        expectedError: 'toolchain',
      ),
    );
    await run(
      'missing-target-never-installs',
      (fixture) => exercise(
        repository,
        fixture,
        linux,
        BuildConfiguration.debug,
        missingTarget: true,
        expectedError: 'target',
      ),
    );
    await run(
      'missing-workspace-pin',
      (fixture) => exercise(
        repository,
        fixture,
        linux,
        BuildConfiguration.debug,
        missingPin: true,
        expectedError: 'pin',
      ),
    );
    for (final channel in ['stable', '1.95']) {
      await run(
        'inexact-workspace-pin-$channel',
        (fixture) => exercise(
          repository,
          fixture,
          linux,
          BuildConfiguration.debug,
          badPin: '[toolchain]\nchannel="$channel"\n',
          expectedError: 'pin',
        ),
      );
    }
    await run(
      'missing-NDK-never-installs',
      (fixture) => exercise(
        repository,
        fixture,
        android,
        BuildConfiguration.debug,
        missingNdk: true,
        expectedError: 'NDK',
      ),
    );
    await run(
      'provisioned-zigbuild-is-not-reinstalled',
      (fixture) => exercise(
        repository,
        fixture,
        linux,
        BuildConfiguration.debug,
        zig: true,
      ),
    );
    await run(
      'missing-zigbuild-never-installs',
      (fixture) => exercise(
        repository,
        fixture,
        linux,
        BuildConfiguration.debug,
        zig: true,
        missingZig: true,
        expectedError: 'zigbuild',
      ),
    );
    for (final entry in {
      'sdk': 'androidSdkPath',
      'ndkVersion': 'androidNdkVersion',
      'minSdk': 'androidMinSdkVersion',
    }.entries) {
      await run(
        'retained-missing-${entry.key}-control',
        (fixture) => exercise(
          repository,
          fixture,
          android,
          BuildConfiguration.debug,
          missingAndroidSetting: entry.key,
          expectedError: entry.value,
        ),
      );
    }
    await run('retained-stable-inventory-control', (_) async {
      testRunCommandOverride = (command) => TestRunCommandResult(
        stdout: command.arguments[0] == 'toolchain'
            ? 'stable-x86_64-unknown-linux-gnu'
            : linux.rust,
      );
      try {
        final rustup = Rustup();
        require(
          rustup.installedTargets('stable')!.contains(linux.rust),
          'stable-inventory-lost',
        );
        require(
          rustup.installedTargets('missing-release') == null,
          'missing-inventory-false-positive',
        );
      } finally {
        testRunCommandOverride = null;
      }
    });
  } finally {
    testRunCommandOverride = null;
    root.deleteSync(recursive: true);
  }
  stdout.writeln('CARGOKIT_TOOLCHAIN $passed passed, $failed failed');
  exitCode = failed == 0 ? 0 : 1;
}
