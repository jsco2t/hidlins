/// Relocate a restart snapshot's vault path while preserving its other fields.
String relocateRestartRegistry(String original, String vaultPath) {
  final pathField = RegExp(r'^path = ".*"$', multiLine: true);
  if (!pathField.hasMatch(original)) {
    throw StateError('restart-registry-path-missing');
  }
  return original.replaceFirst(pathField, 'path = "$vaultPath"');
}
