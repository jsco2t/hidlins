sealed class PlatformResult<T> {
  const PlatformResult();
}

final class PlatformSuccess<T> extends PlatformResult<T> {
  const PlatformSuccess(this.value);

  final T value;
}

final class PlatformCanceled<T> extends PlatformResult<T> {
  const PlatformCanceled();
}

final class PlatformDenied<T> extends PlatformResult<T> {
  const PlatformDenied();
}

final class PlatformStale<T> extends PlatformResult<T> {
  const PlatformStale();
}

final class PlatformUnsupported<T> extends PlatformResult<T> {
  const PlatformUnsupported();
}

final class PlatformFailure<T> extends PlatformResult<T> {
  const PlatformFailure(this.code);

  /// Stable, non-payload native code. Native messages are intentionally not
  /// retained because they are outside the reviewed secret-redaction boundary.
  final String code;
}

final class PlatformUnit {
  const PlatformUnit._();
}

const platformUnit = PlatformUnit._();
