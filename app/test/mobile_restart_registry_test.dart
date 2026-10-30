import 'package:flutter_test/flutter_test.dart';

import '../integration_test/support/restart_registry.dart';

void main() {
  const androidPath = '/data/user/0/app.hidlins/files/scenario/authority.kdbx';
  const registry =
      '''[[vaults]]
name = "authority"
path = "$androidPath"
sealed_identity = "opaque-sealed-fixture"
''';

  test(
    'Android restart accepts the same app path and preserves the registry',
    () {
      expect(relocateRestartRegistry(registry, androidPath), registry);
    },
  );

  test(
    'relocation preserves sealed identity and all unrelated registry fields',
    () {
      const newPath = '/new-container/scenario/authority.kdbx';
      expect(
        relocateRestartRegistry(registry, newPath),
        registry.replaceFirst(androidPath, newPath),
      );
    },
  );

  test('a missing vault path still fails closed', () {
    expect(
      () => relocateRestartRegistry('name = "authority"\n', androidPath),
      throwsStateError,
    );
  });
}
