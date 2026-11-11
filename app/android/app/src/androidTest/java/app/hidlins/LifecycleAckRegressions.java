package app.hidlins;

import static app.hidlins.DeviceAssertions.assertEquals;
import static app.hidlins.DeviceAssertions.assertFalse;
import static app.hidlins.DeviceAssertions.assertTrue;

import android.app.Instrumentation;
import android.content.Intent;
import io.flutter.plugin.common.BinaryMessenger;
import io.flutter.plugin.common.MethodCall;
import io.flutter.plugin.common.StandardMethodCodec;
import java.nio.ByteBuffer;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.atomic.AtomicReference;
import java.util.function.Consumer;

/** Real Android views and the actual registered shipping method-channel handler. */
final class LifecycleAckRegressions {
    private static final String CHANNEL = "app.hidlins/lifecycle";

    private static final class Messenger implements BinaryMessenger {
        final Map<String, BinaryMessageHandler> handlers = new HashMap<>();
        final List<MethodCall> events = new ArrayList<>();

        private static ByteBuffer readable(ByteBuffer encoded) {
            ByteBuffer bytes = encoded.duplicate();
            bytes.flip();
            return bytes;
        }

        @Override
        public void send(String channel, ByteBuffer message) {
            send(channel, message, null);
        }

        @Override
        public void send(String channel, ByteBuffer message, BinaryReply callback) {
            if (CHANNEL.equals(channel)) {
                events.add(StandardMethodCodec.INSTANCE.decodeMethodCall(readable(message)));
            }
            if (callback != null) callback.reply(null);
        }

        @Override
        public void setMessageHandler(String channel, BinaryMessageHandler handler) {
            if (handler == null) handlers.remove(channel);
            else handlers.put(channel, handler);
        }

        void deliver(Map<String, Object> arguments) {
            ByteBuffer encoded = StandardMethodCodec.INSTANCE.encodeMethodCall(
                    new MethodCall("reportState", arguments));
            handlers.get(CHANNEL).onMessage(readable(encoded), ignored -> {});
        }

        Map<String, Object> latestObservation() {
            return new HashMap<>(events.get(events.size() - 1).arguments());
        }
    }

    private static final class Fixture {
        final SnapshotCoverController cover = new SnapshotCoverController();
        final Messenger messenger = new Messenger();
        final HidlinsPlatformServices services;

        Fixture(MainActivity activity) {
            services = new HidlinsPlatformServices(activity, cover);
            services.register(messenger);
            cover.show(activity);
        }

        Map<String, Object> observe(String state) {
            services.reportLifecycle(state);
            return messenger.latestObservation();
        }

        void acknowledge(Map<String, Object> observation) {
            Map<String, Object> arguments = new HashMap<>(observation);
            arguments.put("acknowledged", true);
            arguments.put("lockState", "locked");
            messenger.deliver(arguments);
        }
    }

    private static void run(Instrumentation instrumentation, Consumer<Fixture> test) {
        MainActivity activity = (MainActivity) instrumentation.startActivitySync(
                new Intent(instrumentation.getTargetContext(), MainActivity.class)
                        .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
        instrumentation.waitForIdleSync();
        AtomicReference<Throwable> failure = new AtomicReference<>();
        instrumentation.runOnMainSync(() -> {
            Fixture fixture = null;
            try {
                fixture = new Fixture(activity);
                test.accept(fixture);
            } catch (Throwable error) {
                // Keep a failing assertion out of the Android main looper.
                failure.set(error);
            } finally {
                if (fixture != null) fixture.services.close();
            }
        });
        if (failure.get() != null) throw new AssertionError("native lifecycle regression", failure.get());
    }

    static void staleAfterPause(Instrumentation instrumentation) {
        run(instrumentation, fixture -> {
            Map<String, Object> resumed = fixture.observe("resumed");
            fixture.observe("paused");
            fixture.acknowledge(resumed);
            assertTrue(fixture.cover.isVisible());
        });
    }

    static void staleAcrossResumeABA(Instrumentation instrumentation) {
        run(instrumentation, fixture -> {
            Map<String, Object> firstResume = fixture.observe("resumed");
            fixture.observe("paused");
            Map<String, Object> latestResume = fixture.observe("resumed");
            fixture.acknowledge(firstResume);
            assertTrue(fixture.cover.isVisible());
            fixture.acknowledge(latestResume);
            assertFalse(fixture.cover.isVisible());
        });
    }

    static void unnumberedCannotHide(Instrumentation instrumentation) {
        run(instrumentation, fixture -> {
            Map<String, Object> resumed = fixture.observe("resumed");
            resumed.remove("observationToken");
            fixture.acknowledge(resumed);
            assertTrue(fixture.cover.isVisible());
        });
    }

    static void currentAcknowledgementHides(Instrumentation instrumentation) {
        run(instrumentation, fixture -> {
            Map<String, Object> resumed = fixture.observe("resumed");
            assertTrue(fixture.cover.isVisible());
            fixture.acknowledge(resumed);
            assertFalse(fixture.cover.isVisible());
        });
    }

    static void bootstrapReplaysCurrent(Instrumentation instrumentation) {
        run(instrumentation, fixture -> {
            Map<String, Object> paused = fixture.observe("paused");
            int before = fixture.messenger.events.size();
            fixture.messenger.deliver(Map.of("requestObservation", true));
            assertEquals(before + 1, fixture.messenger.events.size());
            assertEquals(paused, fixture.messenger.latestObservation());
            assertTrue(fixture.cover.isVisible());
        });
    }
}
