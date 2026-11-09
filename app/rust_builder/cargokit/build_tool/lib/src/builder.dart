/// This is copied from Cargokit (which is the official way to use it currently)
/// Details: https://fzyzcjy.github.io/flutter_rust_bridge/manual/integrate/builtin

import 'dart:io';

import 'package:collection/collection.dart';
import 'package:logging/logging.dart';
import 'package:path/path.dart' as path;
import 'package:toml/toml.dart';

import 'android_environment.dart';
import 'cargo.dart';
import 'environment.dart';
import 'options.dart';
import 'rustup.dart';
import 'target.dart';
import 'util.dart';
import 'vendor_cache.dart';

final _log = Logger('builder');

enum BuildConfiguration {
  debug,
  release,
  profile,
}

extension on BuildConfiguration {
  bool get isDebug => this == BuildConfiguration.debug;
  String get rustName => switch (this) {
        BuildConfiguration.debug => 'debug',
        BuildConfiguration.release => 'release',
        BuildConfiguration.profile => 'release',
      };
}

class BuildException implements Exception {
  final String message;

  BuildException(this.message);

  @override
  String toString() {
    return 'BuildException: $message';
  }
}

class BuildEnvironment {
  final BuildConfiguration configuration;
  final CargokitCrateOptions crateOptions;
  final String targetTempDir;
  final String manifestDir;
  final CrateInfo crateInfo;

  final bool isAndroid;
  final String? androidSdkPath;
  final String? androidNdkVersion;
  final int? androidMinSdkVersion;
  final String? javaHome;

  final String? glibcVersion;

  BuildEnvironment({
    required this.configuration,
    required this.crateOptions,
    required this.targetTempDir,
    required this.manifestDir,
    required this.crateInfo,
    required this.isAndroid,
    this.androidSdkPath,
    this.androidNdkVersion,
    this.androidMinSdkVersion,
    this.javaHome,
    this.glibcVersion,
  });

  static BuildConfiguration parseBuildConfiguration(String value) {
    // XCode configuration adds the flavor to configuration name.
    final firstSegment = value.split('-').first;
    final buildConfiguration = BuildConfiguration.values.firstWhereOrNull(
      (e) => e.name == firstSegment,
    );
    if (buildConfiguration == null) {
      _log.warning('Unknown build configuraiton $value, will assume release');
      return BuildConfiguration.release;
    }
    return buildConfiguration;
  }

  static BuildEnvironment fromEnvironment({
    required bool isAndroid,
  }) {
    final buildConfiguration =
        parseBuildConfiguration(Environment.configuration);
    final manifestDir = Environment.manifestDir;
    final crateOptions = CargokitCrateOptions.load(
      manifestDir: manifestDir,
    );
    final crateInfo = CrateInfo.load(manifestDir);
    return BuildEnvironment(
      configuration: buildConfiguration,
      crateOptions: crateOptions,
      targetTempDir: Environment.targetTempDir,
      manifestDir: manifestDir,
      crateInfo: crateInfo,
      isAndroid: isAndroid,
      androidSdkPath: isAndroid ? Environment.sdkPath : null,
      androidNdkVersion: isAndroid ? Environment.ndkVersion : null,
      androidMinSdkVersion:
          isAndroid ? int.parse(Environment.minSdkVersion) : null,
      javaHome: isAndroid ? Environment.javaHome : null,
    );
  }
}

class RustBuilder {
  final Target target;
  final BuildEnvironment environment;

  RustBuilder({
    required this.target,
    required this.environment,
  });

  void prepare(Rustup rustup) {
    // Hidlins patch: normal builds verify prerequisites; only explicit
    // setup commands may provision compilers, targets or SDK tools.
    final toolchain = _toolchain;
    final installed = rustup.installedTargets(toolchain);
    if (installed == null) {
      throw BuildException(
        'Rust toolchain $toolchain is not installed; run make toolchain before building.',
      );
    }
    if (!installed.contains(target.rust)) {
      throw BuildException(
        'Rust target ${target.rust} is not installed for $toolchain; run make toolchain before building.',
      );
    }
    if (environment.glibcVersion != null) {
      try {
        final probe = runCommand('rustup', [
          'run',
          toolchain,
          'cargo',
          'zigbuild',
          '--version',
        ]);
        if (probe.exitCode != 0) {
          throw BuildException(
            'cargo-zigbuild must be explicitly provisioned before this cross-build.',
          );
        }
      } on CommandFailedException {
        throw BuildException(
          'cargo-zigbuild must be explicitly provisioned before this cross-build.',
        );
      }
    }
  }

  CargoBuildOptions? get _buildOptions =>
      environment.crateOptions.cargo[environment.configuration];

  // Hidlins patch: the workspace exact pin overrides Cargokit's mutable
  // channel preference in every profile. Keep one selection through prepare/build.
  late final String _toolchain = _workspaceToolchain();
  late final Directory _workspaceRoot;

  String _workspaceToolchain() {
    var directory = Directory(environment.manifestDir).absolute;
    while (true) {
      final file = File(path.join(directory.path, 'rust-toolchain.toml'));
      if (file.existsSync()) {
        try {
          final data = TomlDocument.parse(file.readAsStringSync()).toMap();
          final table = data['toolchain'];
          final channel = table is Map ? table['channel'] : null;
          if (channel is! String ||
              !RegExp(r'^\d+\.\d+\.\d+$').hasMatch(channel)) {
            throw BuildException(
              'The workspace toolchain pin must name an exact Rust release.',
            );
          }
          _workspaceRoot = directory;
          return channel;
        } on BuildException {
          rethrow;
        } catch (_) {
          throw BuildException(
            'Cannot read the workspace toolchain pin at ${file.path}.',
          );
        }
      }
      final parent = directory.parent;
      if (parent.path == directory.path) break;
      directory = parent;
    }
    throw BuildException(
      'Missing workspace toolchain pin (rust-toolchain.toml).',
    );
  }

  /// Returns the path of directory containing build artifacts.
  Future<String> build() async {
    final toolchain = _toolchain;
    final extraArgs = _buildOptions?.flags ?? [];
    // Hidlins patch: the api crate's default feature enables desktop
    // clipboard backends. Mobile uses the audited native transfer boundary,
    // so compiling desktop/Wayland code for iOS or Android is both incorrect
    // and unsupported by those targets.
    final mobileArgs = target.android != null ||
            target.darwinPlatform == 'iphoneos' ||
            target.darwinPlatform == 'iphonesimulator'
        ? ['--no-default-features']
        : <String>[];
    // Hidlins patch: enable the existing OS lock/sleep adapter in Linux
    // desktop app builds; Android triples also contain linux, so use the
    // Flutter desktop target identity and scope the feature to our API crate.
    final linuxArgs = environment.crateInfo.packageName == 'hidlins-api' &&
            (target.flutter?.startsWith('linux-') ?? false)
        ? ['--features', 'logind']
        : <String>[];
    final manifestPath = path.join(environment.manifestDir, 'Cargo.toml');
    // Hidlins patch: --offline --locked enforces the vendored/pinned
    // build posture. Cargo resolves from vendor/ via .cargo/config.toml.
    final buildEnvironment = await _buildEnvironment();
    // Hidlins patch: metadata must see the same Cargo configuration as the
    // compile, including separate intermediate directories and config files.
    final metadataCargo = <String>[
      Rustup.executablePath() ?? (throw RustupNotFoundException()),
      'run',
      toolchain,
      'cargo',
    ];
    for (var index = 0; index < extraArgs.length; index++) {
      final argument = extraArgs[index];
      if (argument == '--config' && index + 1 < extraArgs.length) {
        metadataCargo.addAll([argument, extraArgs[++index]]);
      } else if (argument.startsWith('--config=')) {
        metadataCargo.add(argument);
      }
    }
    return VendorCache.build(
      sourceRoot: _workspaceRoot.path,
      manifestDirectory: environment.manifestDir,
      targetDirectory: environment.targetTempDir,
      cargo: metadataCargo,
      compile: () {
        runCommand(
          'rustup',
          [
            'run',
            toolchain,
            'cargo',
            (target.android == null && environment.glibcVersion != null)
                ? 'zigbuild'
                : 'build',
            '--offline',
            '--locked',
            ...mobileArgs,
            ...linuxArgs,
            ...extraArgs,
            '--manifest-path',
            manifestPath,
            '-p',
            environment.crateInfo.packageName,
            if (!environment.configuration.isDebug) '--release',
            '--target',
            target.rust +
                ((target.android == null && environment.glibcVersion != null)
                    ? '.${environment.glibcVersion!}'
                    : ""),
            '--target-dir',
            environment.targetTempDir,
          ],
          // Hidlins patch: Cargo discovers .cargo/config.toml from its working
          // directory, not --manifest-path. Start at the workspace crate so iOS
          // pod builds use the repository's vendored-source replacement.
          workingDirectory: environment.manifestDir,
          environment: buildEnvironment,
        );
        return path.join(
          environment.targetTempDir,
          target.rust,
          environment.configuration.rustName,
        );
      },
    );
  }

  Future<Map<String, String>> _buildEnvironment() async {
    if (target.android == null) {
      return {};
    } else {
      final sdkPath = environment.androidSdkPath;
      final ndkVersion = environment.androidNdkVersion;
      final minSdkVersion = environment.androidMinSdkVersion;
      if (sdkPath == null) {
        throw BuildException('androidSdkPath is not set');
      }
      if (ndkVersion == null) {
        throw BuildException('androidNdkVersion is not set');
      }
      if (minSdkVersion == null) {
        throw BuildException('androidMinSdkVersion is not set');
      }
      final env = AndroidEnvironment(
        sdkPath: sdkPath,
        ndkVersion: ndkVersion,
        minSdkVersion: minSdkVersion,
        targetTempDir: environment.targetTempDir,
        target: target,
      );
      // Hidlins patch: SDK setup is explicit, never a side effect of a build.
      if (!env.ndkIsInstalled()) {
        throw BuildException(
          'Pinned Android NDK $ndkVersion is missing; run make android-ndk-toolchain before building.',
        );
      }
      return env.buildEnvironment();
    }
  }
}
