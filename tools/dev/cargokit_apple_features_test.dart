import 'dart:convert';
import 'dart:io';

import 'package:build_tool/src/builder.dart';
import 'package:build_tool/src/cargo.dart';
import 'package:build_tool/src/options.dart';
import 'package:build_tool/src/target.dart';
import 'package:build_tool/src/util.dart';

void require(bool value, String code) {
  if (!value) throw StateError(code);
}

Set<String> features(List<String> args) {
  final values = <String>{};
  for (var index = 0; index < args.length; index++) {
    if (args[index] == '--features' && index + 1 < args.length) {
      values.addAll(args[index + 1].split(','));
    } else if (args[index].startsWith('--features=')) {
      values.addAll(args[index].substring('--features='.length).split(','));
    }
  }
  return values;
}

String argument(List<String> args, String name) {
  final index = args.indexOf(name);
  require(index >= 0 && index + 1 < args.length, 'required-argument-missing');
  return args[index + 1];
}

Future<void> exercise(
  Directory repository,
  Directory fixture,
  Target target,
  BuildConfiguration configuration, {
  String package = 'hidlins-api',
  required bool enforceApple,
}) async {
  final manifest = Directory('${repository.path}/crates/$package');
  require(
    File('${manifest.path}/Cargo.toml').existsSync(),
    'real-manifest-missing',
  );
  final artifacts = Directory('${fixture.path}/artifacts')..createSync();
  final sdk = Directory('${fixture.path}/sdk');
  const ndkVersion = '29.0.14524491';
  final host = Platform.isMacOS ? 'darwin-x86_64' : 'linux-x86_64';
  final ndk = Directory('${sdk.path}/ndk/$ndkVersion')
    ..createSync(recursive: true);
  File('${ndk.path}/package.xml').writeAsStringSync('<fixture/>');
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

    return TestRunCommandResult();
  };
  try {
    final builder = RustBuilder(
      target: target,
      environment: BuildEnvironment(
        configuration: configuration,
        crateOptions: CargokitCrateOptions(
          cargo: {
            configuration: CargoBuildOptions(
              toolchain: Toolchain.stable,
              flags: ['--color', 'never'],
            ),
          },
        ),
        targetTempDir: artifacts.path,
        manifestDir: manifest.path,
        crateInfo: CrateInfo.load(manifest.path),
        isAndroid: target.android != null,
        androidSdkPath: sdk.path,
        androidNdkVersion: ndkVersion,
        androidMinSdkVersion: 29,
      ),
    );
    await builder.build();
    final cargoBuilds = commands
        .where(
          (command) =>
              command.executable == 'rustup' &&
              command.arguments.length >= 4 &&
              command.arguments[0] == 'run' &&
              command.arguments[2] == 'cargo' &&
              command.arguments[3] == 'build',
        )
        .toList();
    require(cargoBuilds.length == 1, 'expected-one-cargo-build');
    final command = cargoBuilds.single;
    final args = command.arguments;
    stdout.writeln(
      'COMMAND ${target.rust}/${configuration.name}/$package ${jsonEncode(args)}',
    );
    final pin = RegExp(r'channel = "([0-9.]+)"')
        .firstMatch(
          File('${repository.path}/rust-toolchain.toml').readAsStringSync(),
        )!
        .group(1);
    require(args[1] == pin, 'exact-workspace-toolchain-changed');
    require(command.executable == 'rustup', 'cargo-executable-changed');
    require(
      args.length >= 4 &&
          args[0] == 'run' &&
          args[2] == 'cargo' &&
          args[3] == 'build',
      'cargo-dispatch-changed',
    );
    // Compiler pinning is owned by the separate infrastructure repair. This
    // regression protects feature selection without pinning mutable "stable".
    require(
      args.contains('--offline') && args.contains('--locked'),
      'offline-or-lock-guard-missing',
    );
    require(
      command.workingDirectory == manifest.path,
      'manifest-working-directory-changed',
    );
    require(
      argument(args, '--manifest-path') == '${manifest.path}/Cargo.toml',
      'manifest-path-changed',
    );
    require(argument(args, '-p') == package, 'package-selection-changed');
    require(
      argument(args, '--target') == target.rust,
      'target-selection-changed',
    );
    require(
      argument(args, '--target-dir') == artifacts.path,
      'artifact-root-changed',
    );
    require(argument(args, '--color') == 'never', 'user-extra-flags-lost');
    require(
      args.contains('--release') == (configuration != BuildConfiguration.debug),
      'profile-selection-changed',
    );
    final mobile =
        target.android != null ||
        target.darwinPlatform == 'iphoneos' ||
        target.darwinPlatform == 'iphonesimulator';
    require(
      args.contains('--no-default-features') == mobile,
      'mobile-defaults-boundary-changed',
    );
    final selected = features(args);
    final appleApp =
        package == 'hidlins-api' && target.darwinPlatform == 'macosx';
    if (enforceApple) {
      require(
        selected.contains('iokit') == appleApp,
        appleApp ? 'macos-lock-feature-missing' : 'macos-lock-feature-leaked',
      );
    }
    final linuxApp =
        package == 'hidlins-api' &&
        (target.flutter?.startsWith('linux-') ?? false);
    require(
      selected.contains('logind') == linuxApp,
      linuxApp ? 'linux-lock-feature-missing' : 'linux-lock-feature-leaked',
    );
    if (mobile)
      require(
        !selected.contains('desktop') && !selected.contains('iokit'),
        'desktop-feature-leaked-into-mobile',
      );
  } finally {
    testRunCommandOverride = null;
  }
}

Future<void> main(List<String> arguments) async {
  require(arguments.length == 2, 'repository-and-mode-required');
  require(['characterize', 'test'].contains(arguments[1]), 'unknown-mode');
  final enforceApple = arguments[1] == 'test';
  final repository = Directory(arguments.first).absolute;
  final root = Directory.systemTemp.createTempSync(
    'hidlins-cargokit-features-',
  );
  var failed = 0;
  var passed = 0;
  final targets = <Target>[
    Target.forDarwin(platformName: 'macosx', darwinAarch: 'arm64')!,
    Target.forDarwin(platformName: 'macosx', darwinAarch: 'x86_64')!,
    Target.forFlutterName('linux-x64')!,
    Target.forFlutterName('linux-arm64')!,
    Target.forFlutterName('android-x64')!,
    Target.forFlutterName('android-arm64')!,
    Target.forDarwin(platformName: 'iphoneos', darwinAarch: 'arm64')!,
    Target.forDarwin(platformName: 'iphonesimulator', darwinAarch: 'arm64')!,
    Target.forDarwin(platformName: 'iphonesimulator', darwinAarch: 'x86_64')!,
  ];
  try {
    var ordinal = 0;
    for (final configuration in BuildConfiguration.values) {
      for (final target in [
        ...targets,
        Target.forDarwin(platformName: 'macosx', darwinAarch: 'arm64')!,
      ]) {
        final otherCrate = ordinal % (targets.length + 1) == targets.length;
        final label =
            '${target.rust}/${configuration.name}/${otherCrate ? 'other-crate' : 'app'}';
        final directory = Directory('${root.path}/case-${ordinal++}')
          ..createSync();
        try {
          await exercise(
            repository,
            directory,
            target,
            configuration,
            package: otherCrate ? 'hidlins-genpw' : 'hidlins-api',
            enforceApple: enforceApple,
          );
          passed++;
          stdout.writeln('PASS $label');
        } on StateError catch (error) {
          failed++;
          stdout.writeln('FAIL $label ${error.message}');
        } catch (error) {
          failed++;
          stdout.writeln('FIXTURE_ERROR $label ${error.runtimeType}');
        }
      }
    }
  } finally {
    testRunCommandOverride = null;
    root.deleteSync(recursive: true);
  }
  stdout.writeln('CARGOKIT_APPLE_FEATURES $passed passed, $failed failed');
  exitCode = failed == 0 ? 0 : 1;
}
