// SDKWORK-TLS-TRUST-TEMPLATE v1
//
// Canonical HTTPS trust layer for every SDKWork Flutter application.
//
// This file is generated from `sdkwork-specs/templates/flutter/sdkwork_tls_trust.dart`
// and vendored into the application root at `lib/bootstrap/sdkwork_tls_trust.dart`.
// Do not edit a vendored copy: change the template, then run
// `node <sdkwork-specs>/tools/check-flutter-tls-trust.mjs --workspace <workspace> --fix`.
//
// Contract (FLUTTER_APP_MOBILE_ARCHITECTURE_SPEC.md section 9.1):
// - Bootstrap installs this layer before any HTTP client is created.
// - Release builds are always strict: an untrusted certificate is never accepted.
// - Development/debug builds may accept self-signed certificates, scoped by an
//   explicit host allowlist and always announced through `dart:developer`.
// - Additional private CAs are added to the trusted roots for every build mode.
//
// The file depends on `dart:io` only, so it can be unit tested without a
// Flutter binding. Diagnostics use `print` on purpose: `dart:developer` messages
// never reach `flutter run` stdout or `adb logcat`, and a trust decision that
// cannot be seen in the terminal is not reviewable.

// ignore_for_file: avoid_print

import 'dart:convert';
import 'dart:io';

/// Trust policy for outbound HTTPS traffic.
enum SdkworkTlsTrustMode {
  /// Only certificates chaining to trusted roots, plus configured extra CAs.
  strict,

  /// Non-production only: also accept self-signed / untrusted chains for
  /// allowlisted hosts. Always refused in release builds.
  insecureDev,
}

/// Immutable TLS trust configuration resolved from the environment.
class SdkworkTlsTrustConfig {
  const SdkworkTlsTrustConfig({
    required this.mode,
    required this.allowedHosts,
    required this.extraCaPem,
    required this.isReleaseMode,
    required this.modeSource,
    required this.warnings,
  });

  /// Effective trust mode after release-mode coercion.
  final SdkworkTlsTrustMode mode;

  /// Hosts allowed to present an untrusted certificate.
  ///
  /// Entries are case-insensitive; `*.example.com` matches any subdomain.
  /// An empty list with [insecureDev] means "any host" and is announced as a
  /// warning at install time.
  final List<String> allowedHosts;

  /// PEM bundle added to the trusted roots for every trust mode.
  final String? extraCaPem;

  /// Whether the application runs in release (or profile) mode.
  final bool isReleaseMode;

  /// Where the mode came from, for startup diagnostics.
  final String modeSource;

  /// Environment problems discovered while resolving the configuration.
  final List<String> warnings;

  /// Whether the configuration still permits untrusted certificates.
  bool get allowsUntrustedCertificate =>
      mode == SdkworkTlsTrustMode.insecureDev && !isReleaseMode;

  /// Returns `true` when [host] may present an untrusted certificate.
  ///
  /// This only answers the host question; the caller must still check
  /// [allowsUntrustedCertificate].
  bool isHostAllowed(String host) {
    if (allowedHosts.isEmpty) {
      return true;
    }
    final normalized = host.trim().toLowerCase();
    for (final entry in allowedHosts) {
      final pattern = entry.trim().toLowerCase();
      if (pattern.isEmpty) {
        continue;
      }
      if (pattern == normalized) {
        return true;
      }
      if (pattern.startsWith('*.')) {
        final suffix = pattern.substring(1);
        if (normalized.endsWith(suffix) && normalized.length > suffix.length) {
          return true;
        }
      }
    }
    return false;
  }

  /// Resolves the configuration from the compile-time dart-defines and the
  /// process environment (desktop and tests).
  ///
  /// Compile-time `SDKWORK_TLS_*` values are the canonical Flutter input; a
  /// non-empty process environment value overrides them so desktop runs and
  /// tests can select a policy at launch.
  factory SdkworkTlsTrustConfig.fromEnvironment({
    required bool isReleaseMode,
    Map<String, String>? environment,
  }) {
    final env = environment ?? Platform.environment;
    String pick(String key, String compiled) {
      final runtime = env[key];
      if (runtime != null && runtime.trim().isNotEmpty) {
        return runtime.trim();
      }
      return compiled.trim();
    }

    final warnings = <String>[];
    final rawMode = pick('SDKWORK_TLS_TRUST_MODE', _compiledTrustMode);
    final source = rawMode.isEmpty
        ? (isReleaseMode ? 'release-default' : 'debug-default')
        : 'environment';
    final requested = _parseMode(rawMode, isReleaseMode, warnings);
    final mode = isReleaseMode ? SdkworkTlsTrustMode.strict : requested;
    if (isReleaseMode && requested == SdkworkTlsTrustMode.insecureDev) {
      warnings.add(
        'SDKWORK_TLS_TRUST_MODE=insecureDev was requested in a release build and '
        'has been ignored: untrusted certificates are never accepted in release.',
      );
    }

    final hosts = pick('SDKWORK_TLS_TRUST_HOSTS', _compiledTrustHosts)
        .split(',')
        .map((value) => value.trim())
        .where((value) => value.isNotEmpty)
        .toList(growable: false);

    var pem = pick('SDKWORK_TLS_TRUST_CA_PEM', _compiledTrustCaPem);
    final pemFile = pick('SDKWORK_TLS_TRUST_CA_FILE', _compiledTrustCaFile);
    if (pem.isEmpty && pemFile.isNotEmpty) {
      try {
        pem = File(pemFile).readAsStringSync();
      } on FileSystemException catch (error) {
        warnings.add('SDKWORK_TLS_TRUST_CA_FILE could not be read: $error');
      }
    }
    if (pem.isNotEmpty && !pem.contains('BEGIN CERTIFICATE')) {
      warnings.add(
        'SDKWORK_TLS_TRUST_CA_PEM does not contain a BEGIN CERTIFICATE block; '
        'it will be ignored.',
      );
      pem = '';
    }

    return SdkworkTlsTrustConfig(
      mode: mode,
      allowedHosts: hosts,
      extraCaPem: pem.isNotEmpty ? pem : null,
      isReleaseMode: isReleaseMode,
      modeSource: source,
      warnings: List<String>.unmodifiable(warnings),
    );
  }

  static SdkworkTlsTrustMode _parseMode(
    String raw,
    bool isReleaseMode,
    List<String> warnings,
  ) {
    switch (raw.toLowerCase()) {
      case '':
        return isReleaseMode
            ? SdkworkTlsTrustMode.strict
            : SdkworkTlsTrustMode.insecureDev;
      case 'strict':
        return SdkworkTlsTrustMode.strict;
      case 'insecure-dev':
      case 'insecured':
      case 'insecuredev':
      case 'self-signed':
        return SdkworkTlsTrustMode.insecureDev;
      default:
        warnings.add(
          'Unrecognized SDKWORK_TLS_TRUST_MODE "$raw"; falling back to '
          '${isReleaseMode ? 'strict' : 'insecure-dev'}.',
        );
        return isReleaseMode
            ? SdkworkTlsTrustMode.strict
            : SdkworkTlsTrustMode.insecureDev;
    }
  }

  /// One-line description used by startup diagnostics.
  String describe() {
    final hosts =
        allowedHosts.isEmpty ? '<any-host>' : allowedHosts.join('|');
    return 'mode=${mode.name} release=$isReleaseMode hosts=$hosts '
        'extraCa=${extraCaPem == null ? 'none' : 'configured'} source=$modeSource';
  }

  static const String _compiledTrustMode =
      String.fromEnvironment('SDKWORK_TLS_TRUST_MODE');
  static const String _compiledTrustHosts =
      String.fromEnvironment('SDKWORK_TLS_TRUST_HOSTS');
  static const String _compiledTrustCaPem =
      String.fromEnvironment('SDKWORK_TLS_TRUST_CA_PEM');
  static const String _compiledTrustCaFile =
      String.fromEnvironment('SDKWORK_TLS_TRUST_CA_FILE');
}

/// Installs the process-wide HTTPS trust policy.
///
/// The layer is applied through [HttpOverrides], so it covers every
/// `dart:io` [HttpClient] created by the application, `package:http`
/// (`IOClient`), `package:dio`, and generated Dart SDK clients. Install it from
/// the first statements of `main`, before any client is constructed.
abstract final class SdkworkTlsTrust {
  static SdkworkTlsTrustConfig? _installed;
  static SecurityContext? _securityContext;

  /// The installed configuration, or `null` when the layer is not installed.
  static SdkworkTlsTrustConfig? get installed => _installed;

  /// Resolves the configuration from the environment and installs it.
  static SdkworkTlsTrustConfig installFromEnvironment({
    required bool isReleaseMode,
    Map<String, String>? environment,
  }) {
    return install(
      SdkworkTlsTrustConfig.fromEnvironment(
        isReleaseMode: isReleaseMode,
        environment: environment,
      ),
    );
  }

  /// First statement of `main`: installs the policy using the Dart AOT product
  /// define as the release signal, which is exactly what `kReleaseMode` reports.
  ///
  /// Kept Flutter-free so the application entrypoint only needs one line and
  /// one relative import.
  static SdkworkTlsTrustConfig installForApp({Map<String, String>? environment}) {
    return installFromEnvironment(
      isReleaseMode: const bool.fromEnvironment('dart.vm.product'),
      environment: environment,
    );
  }

  /// Installs [config] as the process-wide trust policy.
  static SdkworkTlsTrustConfig install(SdkworkTlsTrustConfig config) {
    _installed = config;
    _securityContext = null;
    HttpOverrides.global = _SdkworkTlsHttpOverrides(config);
    for (final warning in config.warnings) {
      _log('warning: $warning');
    }
    if (config.allowsUntrustedCertificate) {
      _log(
        'non-production self-signed trust is active '
        '(${config.describe()}); release builds stay strict.',
      );
    } else {
      _log('strict trust active (${config.describe()}).');
    }
    return config;
  }

  /// Applies the configured policy to an already created [client].
  static HttpClient configureClient(
    HttpClient client,
    SdkworkTlsTrustConfig config,
  ) {
    if (!config.allowsUntrustedCertificate) {
      return client;
    }
    client.badCertificateCallback =
        (X509Certificate certificate, String host, int port) {
      if (!config.isHostAllowed(host)) {
        _log(
          'rejected untrusted certificate for $host:$port '
          '(host is not in SDKWORK_TLS_TRUST_HOSTS).',
        );
        return false;
      }
      _log(
        'accepted untrusted certificate for $host:$port '
        '(subject="${certificate.subject}", issuer="${certificate.issuer}").',
      );
      return true;
    };
    return client;
  }

  /// Builds the [SecurityContext] used for every created client.
  ///
  /// Trusting roots are always enabled. A configured CA bundle is added on top;
  /// when the caller supplied its own context the bundle is added to it so the
  /// caller keeps its own roots.
  static SecurityContext buildSecurityContext(
    SdkworkTlsTrustConfig config, {
    SecurityContext? baseContext,
  }) {
    final pem = config.extraCaPem;
    if (pem == null) {
      return _securityContext ??= baseContext ?? SecurityContext(withTrustedRoots: true);
    }
    if (baseContext != null) {
      baseContext.setTrustedCertificatesBytes(utf8.encode(pem));
      return baseContext;
    }
    final cached = _securityContext;
    if (cached != null) {
      return cached;
    }
    final context = SecurityContext(withTrustedRoots: true);
    context.setTrustedCertificatesBytes(utf8.encode(pem));
    _securityContext = context;
    return context;
  }

  static void _log(String message) {
    print('[sdkwork-tls] $message');
  }
}

class _SdkworkTlsHttpOverrides extends HttpOverrides {
  _SdkworkTlsHttpOverrides(this._config);

  final SdkworkTlsTrustConfig _config;

  @override
  HttpClient createHttpClient(SecurityContext? context) {
    final securityContext =
        SdkworkTlsTrust.buildSecurityContext(_config, baseContext: context);
    final client = super.createHttpClient(securityContext);
    return SdkworkTlsTrust.configureClient(client, _config);
  }
}
