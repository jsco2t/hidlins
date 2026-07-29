import 'dart:typed_data';
import 'package:flutter_test/flutter_test.dart';
import 'package:app/src/bridge/dto.dart';

/// Pins the structural redaction of the secret-bearing Dart DTO.
///
/// freezed normally generates `toString()` implementations that interpolate
/// every field — for `KeyfileRef` that would print keyfile paths and raw key
/// bytes into logs, string interpolation, and exception messages. The
/// redaction is injected at the source: `#[frb(dart_code = ...)]` on the Rust
/// enum (crates/hidlins-api/src/dto.rs) emits a `toString() =>
/// 'KeyfileRef(***)'` override into the generated sealed class, and freezed
/// skips its own interpolating `toString` whenever the annotated class
/// declares one. Because the override lives in the Rust source, it survives
/// `make api-gen` regeneration — `api-gen-check` would catch its loss.
///
/// These tests fail if a future frb/freezed upgrade stops honoring either
/// half of that mechanism, so a regression cannot land silently.
/// `tools/dev/boundary-check.sh` check 6 remains as defense-in-depth against
/// hand-written code that stringifies secret *values* (which no `toString`
/// override can prevent).
void main() {
  group('sensitive DTO toString redaction', () {
    test('KeyfileRef.bytes toString reveals neither bytes nor field names', () {
      final kr = KeyfileRef.bytes(Uint8List.fromList([0xDE, 0xAD, 0xBE, 0xEF]));
      final s = kr.toString();
      expect(s, equals('KeyfileRef(***)'));
      // Dart renders a Uint8List in decimal: 0xDE is 222, 0xAD is 173.
      expect(s, isNot(contains('222')));
      expect(s, isNot(contains('173')));
      expect(s, isNot(contains('field0')));
    });

    test('KeyfileRef.path toString reveals no part of the path', () {
      final kr = KeyfileRef.path('/secret/keyfile.key');
      final s = kr.toString();
      expect(s, equals('KeyfileRef(***)'));
      expect(s, isNot(contains('secret')));
      expect(s, isNot(contains('keyfile.key')));
    });

    test('redaction holds through string interpolation and collections', () {
      final kr = KeyfileRef.bytes(Uint8List.fromList([1, 2, 3]));
      expect('$kr', equals('KeyfileRef(***)'));
      expect([kr].toString(), equals('[KeyfileRef(***)]'));
    });
  });
}
