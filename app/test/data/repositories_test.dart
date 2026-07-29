import 'package:flutter_test/flutter_test.dart';

import 'package:app/src/data/models.dart';
import '../fakes/fake_repositories.dart';

void main() {
  group('entry repository invalidation', () {
    test('create triggers onMutation', () async {
      final repo = FakeEntryRepository();
      var called = false;
      repo.onMutation = () => called = true;
      await repo.createEntry(
        'group-uuid',
        const EntryDraftDto(kind: EntryKindDto.credential, title: 'Test'),
      );
      expect(called, isTrue);
    });

    test('update triggers onMutation', () async {
      final repo = FakeEntryRepository();
      var called = false;
      repo.onMutation = () => called = true;
      await repo.updateEntry('uuid', const EntryEditDto(title: 'New'));
      expect(called, isTrue);
    });

    test('delete triggers onMutation', () async {
      final repo = FakeEntryRepository();
      var called = false;
      repo.onMutation = () => called = true;
      await repo.deleteEntry('uuid');
      expect(called, isTrue);
    });

    test('purge triggers onMutation', () async {
      final repo = FakeEntryRepository();
      var called = false;
      repo.onMutation = () => called = true;
      await repo.purgeEntry('uuid');
      expect(called, isTrue);
    });

    test('move triggers onMutation', () async {
      final repo = FakeEntryRepository();
      var called = false;
      repo.onMutation = () => called = true;
      await repo.moveEntry('uuid', 'group');
      expect(called, isTrue);
    });

    test('createGroup triggers onMutation', () async {
      final repo = FakeEntryRepository();
      var called = false;
      repo.onMutation = () => called = true;
      await repo.createGroup('parent', 'name');
      expect(called, isTrue);
    });

    test('renameGroup triggers onMutation', () async {
      final repo = FakeEntryRepository();
      var called = false;
      repo.onMutation = () => called = true;
      await repo.renameGroup('uuid', 'name');
      expect(called, isTrue);
    });

    test('deleteGroup triggers onMutation', () async {
      final repo = FakeEntryRepository();
      var called = false;
      repo.onMutation = () => called = true;
      await repo.deleteGroup('uuid', GroupDeleteBehavior.recurse);
      expect(called, isTrue);
    });

    test('read operations do not trigger onMutation', () async {
      final repo = FakeEntryRepository();
      var called = false;
      repo.onMutation = () => called = true;
      await repo.vaultTree();
      await repo.entryDetail('uuid');
      await repo.entryHistory('uuid');
      await repo.listTags();
      await repo.listAttachments('uuid');
      expect(called, isFalse);
    });
  });
}
