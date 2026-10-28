import 'dart:typed_data';

import 'package:flutter/rendering.dart' show RenderImage;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:material_ui/material_ui.dart';

import 'package:app/src/data/models.dart';
import 'package:app/src/features/entries/entries_page.dart';
import 'package:app/src/features/lock/unlock_screen.dart';
import 'package:app/src/features/sync/sync_controller.dart';
import 'package:app/src/features/sync/sync_page.dart';
import 'package:app/src/features/vaults/first_run_page.dart';
import 'package:app/src/ui/adaptive_scaffold.dart';
import 'package:app/src/ui/widgets/brand_mark.dart';

import 'helpers/feature_test_helpers.dart';
import 'helpers/golden.dart';

// This recorder tests the harness contract without adding reference images.
// goldens_test.dart continues to use the strict default pixel comparator.
class _CaptureContract extends GoldenFileComparator {
  Size? pixels;
  VoidCallback? onCapture;

  @override
  Future<bool> compare(Uint8List imageBytes, Uri golden) async {
    final header = ByteData.sublistView(imageBytes);
    pixels = Size(
      header.getUint32(16).toDouble(),
      header.getUint32(20).toDouble(),
    );
    onCapture?.call();
    return true;
  }

  @override
  Future<void> update(Uri golden, Uint8List imageBytes) async {
    throw StateError('Harness contract tests must not update raster files');
  }
}

void main() {
  late GoldenFileComparator original;
  late _CaptureContract capture;
  setUp(() {
    original = goldenFileComparator;
    capture = _CaptureContract();
    goldenFileComparator = capture;
  });
  tearDown(() => goldenFileComparator = original);

  for (final size in [
    goldenSizeCompact,
    goldenSizeMedium,
    goldenSizeExpanded,
  ]) {
    for (final brightness in [Brightness.light, Brightness.dark]) {
      testWidgets('viewport and decoded lock mark $size $brightness', (
        tester,
      ) async {
        final harness = TestHarness();
        addTearDown(harness.dispose);
        registerTestVault(harness);
        Size? logicalSize;
        double? dpr;
        Brightness? actualBrightness;
        capture.onCapture = () {
          final context = tester.element(find.byType(UnlockScreen));
          logicalSize = MediaQuery.sizeOf(context);
          dpr = MediaQuery.devicePixelRatioOf(context);
          actualBrightness = Theme.of(context).brightness;
          final render = tester.renderObject<RenderImage>(
            find.descendant(
              of: find.byType(BrandMark),
              matching: find.byType(RawImage),
            ),
          );
          expect(
            render.image,
            isNotNull,
            reason: 'Capture must wait for actual asset decoding',
          );
        };
        await expectGolden(
          tester,
          ProviderScope(
            overrides: harness.overrides,
            child: const UnlockScreen(),
          ),
          name: 'contract',
          size: size,
          brightness: brightness,
        );
        expect(logicalSize, size);
        expect(actualBrightness, brightness);
        expect(dpr, 3);
        expect(capture.pixels, size * 3);
        expect(
          tester.view.physicalSize / tester.view.devicePixelRatio,
          size,
          reason: 'FlutterView and layout must describe the same viewport',
        );
        expect(find.byType(BrandMark), findsOneWidget);
        expect(tester.getSize(find.byType(BrandMark)), const Size.square(88));
        expect(
          tester.getSize(find.byType(TextField)).width,
          lessThanOrEqualTo(400),
        );
        expect(find.text('Unlock'), findsOneWidget);
        expect(find.byType(CircularProgressIndicator), findsNothing);
      });
    }
  }

  testWidgets('view properties return to test defaults after golden scenes', (
    tester,
  ) async {
    expect(tester.view.physicalSize, const Size(2400, 1800));
    expect(tester.view.devicePixelRatio, 3);
    await tester.pumpWidget(const SizedBox());
    expect(tester.getSize(find.byType(SizedBox)), const Size(800, 600));
  });

  testWidgets('first run keeps all current choices and decodes its mark', (
    tester,
  ) async {
    capture.onCapture = () {
      final render = tester.renderObject<RenderImage>(
        find.descendant(
          of: find.byType(BrandMark),
          matching: find.byType(RawImage),
        ),
      );
      expect(render.image, isNotNull);
    };
    await expectGolden(
      tester,
      FirstRunPage(onChoice: (_) {}),
      name: 'contract',
      size: goldenSizeCompact,
    );
    expect(find.byType(Card), findsNWidgets(3));
    expect(find.text('Import paired vault'), findsOneWidget);
    expect(tester.getSize(find.byType(BrandMark)), const Size.square(88));
    final choices = tester.widgetList<ListTile>(find.byType(ListTile));
    expect(choices.every((choice) => choice.onTap != null), isTrue);
  });

  testWidgets(
    'entries fixture is unlocked and keeps narrow pane room for title',
    (tester) async {
      final harness = TestHarness();
      addTearDown(harness.dispose);
      harness.session.currentLockState = LockEvent.unlocked;
      await expectGolden(
        tester,
        ProviderScope(
          overrides: harness.overrides,
          child: const AdaptiveScaffold(
            selectedIndex: 0,
            onDestinationSelected: _ignore,
            body: EntriesPage(initialUuid: 'entry-1'),
          ),
        ),
        name: 'contract',
        size: goldenSizeExpanded,
      );
      expect(find.byType(NavigationRail), findsOneWidget);
      expect(
        tester.widget<NavigationRail>(find.byType(NavigationRail)).extended,
        isTrue,
      );
      expect(find.text('GitHub'), findsWidgets);
      expect(find.byTooltip('Edit'), findsOneWidget);
      expect(find.text('All entries'), findsOneWidget);
      expect(find.byType(CircularProgressIndicator), findsNothing);
    },
  );

  for (final inFlight in [false, true]) {
    testWidgets(
      'sync fixture depicts current local sync with progress $inFlight',
      (tester) async {
        final harness = TestHarness();
        addTearDown(harness.dispose);
        harness.session.currentLockState = LockEvent.unlocked;
        harness.sync.status = SyncStatusDto(
          configured: inFlight,
          inFlight: inFlight,
          lastOutcome: inFlight ? null : SyncOutcomeDto.merged,
        );
        final before = tester.binding.clock.now();
        await expectGolden(
          tester,
          ProviderScope(overrides: harness.overrides, child: const SyncPage()),
          name: 'contract',
          size: goldenSizeMedium,
          settle: !inFlight,
        );
        expect(find.text('Local network sync'), findsOneWidget);
        expect(find.byType(TextField), findsNothing);
        final context = tester.element(find.byType(SyncPage));
        final state = ProviderScope.containerOf(context)
            .read(syncControllerProvider)
            .requireValue;
        expect(state.inFlight, inFlight);
        if (inFlight) {
          expect(find.byType(CircularProgressIndicator), findsOneWidget);
          expect(
            tester.binding.clock.now().difference(before),
            const Duration(milliseconds: 300),
          );
        } else {
          expect(state.lastOutcome, SyncOutcomeDto.merged);
          expect(find.byType(CircularProgressIndicator), findsNothing);
        }
      },
    );
  }
}

void _ignore(int _) {}
