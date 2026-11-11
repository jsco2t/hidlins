package app.hidlins.releaseprobe;

import android.accessibilityservice.AccessibilityServiceInfo;
import android.app.Activity;
import android.app.Instrumentation;
import android.app.UiAutomation;
import android.content.ClipData;
import android.content.ClipDescription;
import android.content.ClipboardManager;
import android.content.Context;
import android.content.Intent;
import android.graphics.Rect;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import android.view.inputmethod.InputMethodManager;
import android.view.InputDevice;
import android.view.MotionEvent;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.nio.charset.StandardCharsets;
import java.io.File;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;

/** SDK-only test APK. It never changes the shipping app or logs fixture contents. */
@SuppressWarnings("deprecation")
public final class ReleaseClipboardInstrumentation extends Instrumentation {
    private static final String FIXTURE_MASTER = "release-probe-master-not-a-user-secret";
    private static final String FIXTURE_PASSWORD = "release-probe-entry-not-a-user-secret";
    private static final String FIXTURE_ENTRY = "Release clipboard fixture";
    private static final String PROBE_TEXT = "release-array-probe";
    private static final String FOREIGN_TEXT = "release-foreign-probe";
    private Bundle arguments;
    private Activity activity;
    private Object receiver;
    private Method receive;
    private Field ttl;
    private UiAutomation automation;
    private ClipboardManager clipboard;
    private String stage = "launch";
    private boolean scrollForward = true;
    private long beforeEntrySave;
    private String inputField = "FIELD";

    @Override public void onCreate(Bundle supplied) {
        super.onCreate(supplied);
        arguments = supplied == null ? new Bundle() : supplied;
        start();
    }

    @Override public void onStart() {
        boolean passed = false;
        String failure = "SETUP_FAILURE";
        try {
            activity = startActivitySync(new Intent()
                    .setClassName("app.hidlins", "app.hidlins.MainActivity")
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
            long focusDeadline = SystemClock.elapsedRealtime() + 30_000;
            while (!activity.hasWindowFocus() && SystemClock.elapsedRealtime() < focusDeadline) {
                SystemClock.sleep(20);
            }
            require(activity.hasWindowFocus(), "SETUP_FOCUS");
            stage = "receiver";
            receiver = field(activity, requiredArgument("servicesField")).get(activity);
            require(receiver != null, "SETUP_SERVICES");
            receiver = field(receiver, requiredArgument("clipboardField")).get(receiver);
            require(receiver != null, "SETUP_RECEIVER");
            Class<?> nativeType = activity.getClassLoader().loadClass("app.hidlins.HidlinsNative");
            Method consume = null;
            for (Method method : nativeType.getDeclaredMethods()) {
                if (method.getName().equals("consumeClipboard")) consume = method;
            }
            require(consume != null && consume.getParameterTypes().length == 2
                    && consume.getParameterTypes()[1].isInstance(receiver), "SETUP_NATIVE_RECEIVER");
            if ("native".equals(arguments.getString("mode"))) {
                initializeAutomation();
                stage = "native-library";
                probeNative(consume);
                passed = true;
                return;
            }
            stage = "callback-abi";
            try {
                receive = receiver.getClass().getMethod("receive", byte[].class);
            } catch (NoSuchMethodException missing) {
                throw new ProbeFailure("ABI_MISSING");
            }
            require(receive.getReturnType() == boolean.class
                    && !Modifier.isStatic(receive.getModifiers()), "ABI_WRONG_DESCRIPTOR");
            receive.setAccessible(true);
            if (!"abi".equals(arguments.getString("mode", "full"))) {
                initializeAutomation();
                stage = "native-library";
                probeNative(consume);
                stage = "shipping-rust-copy";
                clipboard = (ClipboardManager) activity.getSystemService(Context.CLIPBOARD_SERVICE);
                require(clipboard != null, "SETUP_CLIPBOARD");
                shippingCopy();
                stage = "array-erasure";
                // The original release removes this field too. Never require
                // it before the intended ABI failure has been established.
                ttl = field(receiver, requiredArgument("ttlField"));
                probeArrays();
            }
            passed = true;
        } catch (ProbeFailure known) {
            failure = known.code;
            if ("SETUP_FIELD_FOCUS".equals(failure) || "SETUP_TEXT_NOT_APPLIED".equals(failure)) {
                failure += "_" + inputField;
            }
            if ("SETUP_UI_TARGET".equals(failure) && "entry-editor".equals(stage)) {
                failure += "_" + inputField + editorDiagnostic();
            }
            if ("SETUP_CREATE_COMPLETION".equals(failure)) failure = creationDiagnostic();
            if ("SETUP_ENTRY_SAVE_COMPLETION".equals(failure)) failure = entrySaveDiagnostic();
        } catch (Throwable unexpected) {
            // Throwable messages and UI trees can contain entry contents.
            failure = "SETUP_FAILURE";
        } finally {
            if (clipboard != null) {
                try {
                    onMain(() -> {
                        String current = currentClip();
                        if (PROBE_TEXT.equals(current) || FOREIGN_TEXT.equals(current)
                                || FIXTURE_PASSWORD.equals(current)) clipboard.clearPrimaryClip();
                    });
                } catch (Throwable ignored) { }
            }
            Bundle result = new Bundle();
            result.putString("stream", passed
                    ? "HIDLINS_RELEASE_CLIPBOARD 1 passed, 0 failed\n"
                    : "HIDLINS_RELEASE_CLIPBOARD 0 passed, 1 failed stage=" + stage + " code=" + failure + "\n");
            finish(passed ? Activity.RESULT_OK : Activity.RESULT_CANCELED, result);
        }
    }

    private String creationDiagnostic() {
        // Fixed classifications only. Never print form values, paths or an
        // arbitrary application error while debugging test setup.
        for (String[] error : new String[][] {
                {"Required", "SETUP_CREATE_REQUIRED"},
                {"Passwords do not match", "SETUP_CREATE_PASSWORD_MISMATCH"},
                {"An error occurred", "SETUP_CREATE_APP_ERROR"},
                {"A vault with this name already exists", "SETUP_CREATE_NAME_EXISTS"}}) {
            AccessibilityNodeInfo node = find(error[0], false);
            if (node != null) { node.recycle(); return error[1]; }
        }
        File state = new File(getTargetContext().getFilesDir(), "hidlins/state");
        if (new File(state, "release-clipboard-fixture.kdbx").isFile()) {
            return new File(state, "vaults.toml").isFile()
                    ? "SETUP_CREATED_UI_UNEXPECTED" : "SETUP_CREATE_REGISTRY_INCOMPLETE";
        }
        AccessibilityNodeInfo create = find("Create", false);
        if (create != null) { create.recycle(); return "SETUP_CREATE_FORM_REMAINS"; }
        return "SETUP_CREATE_STILL_WORKING";
    }

    private String entrySaveDiagnostic() {
        for (String[] error : new String[][] {
                {"Required", "SETUP_ENTRY_REQUIRED"},
                {"An error occurred", "SETUP_ENTRY_APP_ERROR"},
                {"Vault locked", "SETUP_ENTRY_SESSION_LOCKED"}}) {
            AccessibilityNodeInfo node = find(error[0], false);
            if (node != null) { node.recycle(); return error[1]; }
        }
        File vault = new File(getTargetContext().getFilesDir(),
                "hidlins/state/release-clipboard-fixture.kdbx");
        if (vault.lastModified() != beforeEntrySave) return "SETUP_ENTRY_SAVED_UI_MISSING";
        AccessibilityNodeInfo save = find("Save", false);
        if (save != null) { save.recycle(); return "SETUP_ENTRY_FORM_REMAINS"; }
        return "SETUP_ENTRY_SAVE_PENDING";
    }

    private void initializeAutomation() {
        stage = "shipping-ready";
        automation = getUiAutomation();
        AccessibilityServiceInfo info = automation.getServiceInfo();
        info.flags |= AccessibilityServiceInfo.FLAG_RETRIEVE_INTERACTIVE_WINDOWS;
        automation.setServiceInfo(info);
        // main.dart creates the welcome UI only after RustLib.init and
        // initApp. This excludes a race with Dart's initial dlopen from the
        // JVM native-binding probe.
        require(waitForLabel("Create a new vault", false, 60_000), "SETUP_SHIPPING_READY");
    }

    private void probeNative(Method consume) throws Exception {
        try {
            require(Integer.valueOf(1).equals(consume.invoke(null, null, receiver)), "NATIVE_NULL_GUARD");
        } catch (InvocationTargetException failed) {
            if (failed.getCause() instanceof UnsatisfiedLinkError) {
                throw new ProbeFailure("NATIVE_LIBRARY_NOT_LINKED");
            }
            throw failed;
        }
    }

    private String requiredArgument(String name) {
        String value = arguments.getString(name);
        require(value != null && !value.isEmpty(), "SETUP_MAPPING");
        return value;
    }

    private static Field field(Object owner, String name) throws ReflectiveOperationException {
        Class<?> type = owner.getClass();
        while (type != null) {
            try {
                Field field = type.getDeclaredField(name);
                field.setAccessible(true);
                return field;
            } catch (NoSuchFieldException absent) {
                type = type.getSuperclass();
            }
        }
        throw new NoSuchFieldException();
    }

    private void shippingCopy() throws Exception {
        stage = "create-vault";
        tap("Create a new vault");
        setText("Vault name", "release-clipboard-fixture");
        setText("Master password", FIXTURE_MASTER);
        setText("Confirm password", FIXTURE_MASTER);
        setText("Type CONFIRM to acknowledge", "CONFIRM");
        tap("Create");
        require(waitForLabel("Unlock", false, 60_000), "SETUP_CREATE_COMPLETION");
        stage = "unlock-vault";
        setText("Master password", FIXTURE_MASTER);
        tap("Unlock");
        tap("New entry");
        stage = "entry-editor";
        setText("Title", FIXTURE_ENTRY);
        // Leave username and URL empty: the detail has exactly one Copy action.
        setText("Password", FIXTURE_PASSWORD);
        beforeEntrySave = new File(getTargetContext().getFilesDir(),
                "hidlins/state/release-clipboard-fixture.kdbx").lastModified();
        tap("Save");
        stage = "entry-save";
        long saveDeadline = SystemClock.elapsedRealtime() + 60_000;
        boolean saved = false;
        do {
            AccessibilityNodeInfo copy = find("Copy", false);
            AccessibilityNodeInfo entry = find(FIXTURE_ENTRY, false);
            saved = copy != null || entry != null;
            if (copy != null) copy.recycle();
            if (entry != null) entry.recycle();
            if (saved) break;
            scroll();
        } while (SystemClock.elapsedRealtime() < saveDeadline);
        require(saved, "SETUP_ENTRY_SAVE_COMPLETION");
        stage = "entry-list";
        if (!waitForLabel("Copy", false, 1500)) tap(FIXTURE_ENTRY);
        stage = "copy-action";
        onMain(() -> clipboard.setPrimaryClip(ClipData.newPlainText("release-sentinel", FOREIGN_TEXT)));
        tap("Copy");
        stage = "rust-copy";
        long deadline = SystemClock.elapsedRealtime() + 15_000;
        boolean copied = false;
        while (SystemClock.elapsedRealtime() < deadline) {
            AtomicReference<Boolean> value = new AtomicReference<>(false);
            onMain(() -> value.set(FIXTURE_PASSWORD.equals(currentClip())));
            if (value.get()) { copied = true; break; }
            SystemClock.sleep(50);
        }
        require(copied, "RUST_COPY_NOT_PUBLISHED");
        onMain(() -> {
            ClipDescription description = clipboard.getPrimaryClipDescription();
            require(description != null && description.getExtras() != null
                    && description.getExtras().getBoolean("android.content.extra.IS_SENSITIVE")
                    && description.getExtras().getString("app.hidlins.clipboard.OWNER") != null,
                    "RUST_COPY_METADATA");
        });
    }

    private void probeArrays() throws Exception {
        onMain(() -> {
            try {
                int originalTtl = ttl.getInt(receiver);
                require(originalTtl >= 1 && originalTtl <= 300, "RUST_COPY_TTL");
                byte[] valid = PROBE_TEXT.getBytes(StandardCharsets.UTF_8);
                require(Boolean.TRUE.equals(receive.invoke(receiver, (Object) valid)), "RECEIVE_REJECTED");
                require(allZero(valid), "ARRAY_NOT_ERASED");
                require(PROBE_TEXT.equals(currentClip()), "RECEIVE_NOT_PUBLISHED");
                clipboard.setPrimaryClip(ClipData.newPlainText("release-sentinel", FOREIGN_TEXT));
                byte[] malformed = {(byte) 0xc3, 0x28};
                require(Boolean.FALSE.equals(receive.invoke(receiver, (Object) malformed)), "INVALID_UTF8_ACCEPTED");
                require(allZero(malformed) && FOREIGN_TEXT.equals(currentClip()), "INVALID_UTF8_ERASURE");
                ttl.setInt(receiver, 0);
                byte[] rejected = PROBE_TEXT.getBytes(StandardCharsets.UTF_8);
                require(Boolean.FALSE.equals(receive.invoke(receiver, (Object) rejected)), "INVALID_TTL_ACCEPTED");
                require(allZero(rejected) && FOREIGN_TEXT.equals(currentClip()), "INVALID_TTL_ERASURE");
                ttl.setInt(receiver, originalTtl);
            } catch (ReflectiveOperationException failure) {
                throw new ProbeFailure("SETUP_CALLBACK_INVOCATION");
            }
        });
    }

    private static boolean allZero(byte[] value) {
        for (byte element : value) if (element != 0) return false;
        return true;
    }

    private String currentClip() {
        ClipData clip = clipboard.getPrimaryClip();
        if (clip == null || clip.getItemCount() == 0) return null;
        CharSequence text = clip.getItemAt(0).getText();
        return text == null ? null : text.toString();
    }

    private static void require(boolean value, String code) {
        if (!value) throw new ProbeFailure(code);
    }

    private static final class ProbeFailure extends RuntimeException {
        private static final long serialVersionUID = 1L;
        final String code;
        ProbeFailure(String code) { this.code = code; }
    }

    private void onMain(Runnable action) throws Exception {
        CountDownLatch complete = new CountDownLatch(1);
        AtomicReference<Throwable> failure = new AtomicReference<>();
        new Handler(Looper.getMainLooper()).post(() -> {
            try { action.run(); } catch (Throwable error) { failure.set(error); }
            finally { complete.countDown(); }
        });
        require(complete.await(5, TimeUnit.SECONDS), "SETUP_MAIN_THREAD_TIMEOUT");
        if (failure.get() instanceof ProbeFailure known) throw known;
        require(failure.get() == null, "SETUP_MAIN_THREAD_ACTION");
    }

    private static boolean matches(AccessibilityNodeInfo node, String wanted) {
        CharSequence[] values = {node.getText(), node.getContentDescription(), node.getHintText(), node.getTooltipText()};
        for (CharSequence value : values) {
            if (value == null) continue;
            for (String line : value.toString().split("\\n")) {
                if (line.equals(wanted) || line.startsWith(wanted + ",")) return true;
            }
        }
        return false;
    }

    private String editorDiagnostic() {
        // Counts of fixed fixture controls only, never arbitrary screen text.
        int editable = 0, title = 0, password = 0, save = 0, create = 0;
        ArrayDeque<AccessibilityNodeInfo> queue = new ArrayDeque<>(roots());
        int visited = 0;
        while (!queue.isEmpty() && visited++ < 5000) {
            AccessibilityNodeInfo node = queue.remove();
            if (node.isEditable()) editable++;
            if (matches(node, "Title")) title++;
            if (matches(node, "Password")) password++;
            if (matches(node, "Save")) save++;
            if (matches(node, "New entry")) create++;
            for (int index = 0; index < node.getChildCount(); index++) {
                AccessibilityNodeInfo child = node.getChild(index);
                if (child != null) queue.add(child);
            }
            node.recycle();
        }
        for (AccessibilityNodeInfo unused : queue) unused.recycle();
        return "_NODES_" + visited + "_FOCUS_" + (activity.hasWindowFocus() ? 1 : 0)
                + "_EDITABLE_" + editable + "_TITLE_" + title + "_PASSWORD_" + password
                + "_SAVE_" + save + "_NEW_" + create;
    }

    private List<AccessibilityNodeInfo> roots() {
        List<AccessibilityNodeInfo> roots = new ArrayList<>();
        for (AccessibilityWindowInfo window : automation.getWindows()) {
            AccessibilityNodeInfo root = window.getRoot();
            if (root != null && root.getPackageName() != null
                    && "app.hidlins".contentEquals(root.getPackageName())) roots.add(root);
            else if (root != null) root.recycle();
            window.recycle();
        }
        return roots;
    }

    private AccessibilityNodeInfo find(String label, boolean editable) {
        return find(label, editable, false);
    }

    private static boolean advertisesClick(AccessibilityNodeInfo node) {
        for (AccessibilityNodeInfo.AccessibilityAction action : node.getActionList()) {
            if (action.getId() == AccessibilityNodeInfo.ACTION_CLICK) return true;
        }
        return false;
    }

    private AccessibilityNodeInfo find(String label, boolean editable, boolean requireClick) {
        ArrayDeque<AccessibilityNodeInfo> queue = new ArrayDeque<>(roots());
        int visited = 0;
        while (!queue.isEmpty() && visited++ < 5000) {
            AccessibilityNodeInfo node = queue.remove();
            if (node.isEnabled() && node.isVisibleToUser()
                    && node.isEditable() == editable && matches(node, label)
                    && (!requireClick || advertisesClick(node))) {
                for (AccessibilityNodeInfo unused : queue) unused.recycle();
                return node;
            }
            for (int index = 0; index < node.getChildCount(); index++) {
                AccessibilityNodeInfo child = node.getChild(index);
                if (child != null) queue.add(child);
            }
            node.recycle();
        }
        for (AccessibilityNodeInfo unused : queue) unused.recycle();
        return null;
    }

    private void scroll() {
        ArrayDeque<AccessibilityNodeInfo> queue = new ArrayDeque<>(roots());
        boolean scrolled = false;
        while (!queue.isEmpty()) {
            AccessibilityNodeInfo node = queue.remove();
            if (!scrolled && node.isScrollable()) {
                scrolled = node.performAction(scrollForward
                        ? AccessibilityNodeInfo.ACTION_SCROLL_FORWARD
                        : AccessibilityNodeInfo.ACTION_SCROLL_BACKWARD);
            }
            for (int index = 0; index < node.getChildCount(); index++) {
                AccessibilityNodeInfo child = node.getChild(index);
                if (child != null) queue.add(child);
            }
            node.recycle();
        }
        if (!scrolled) scrollForward = !scrollForward;
        SystemClock.sleep(scrolled ? 120 : 50);
    }

    private boolean waitForLabel(String label, boolean editable, long timeout) {
        long deadline = SystemClock.elapsedRealtime() + timeout;
        int attempts = 0;
        do {
            AccessibilityNodeInfo node = find(label, editable);
            if (node != null) { node.recycle(); return true; }
            // The phone's unlock form is taller than its viewport. Flutter
            // clips offscreen semantics; waiting alone cannot reveal its
            // bottom button after returning from vault creation.
            if (++attempts % 10 == 0) scroll();
            else SystemClock.sleep(50);
        } while (SystemClock.elapsedRealtime() < deadline);
        return false;
    }

    private AccessibilityNodeInfo await(String label, boolean editable) {
        return await(label, editable, false);
    }

    private AccessibilityNodeInfo await(String label, boolean editable, boolean requireClick) {
        long deadline = SystemClock.elapsedRealtime() + 30_000;
        int scrolls = 0;
        do {
            AccessibilityNodeInfo node = find(label, editable, requireClick);
            if (node != null) return node;
            if (++scrolls % 10 == 0) scroll();
            else SystemClock.sleep(50);
        } while (SystemClock.elapsedRealtime() < deadline);
        throw new ProbeFailure("SETUP_UI_TARGET");
    }

    private void setText(String label, String value) throws Exception {
        inputField = switch (label) {
            case "Vault name" -> "VAULT_NAME";
            case "Master password" -> "MASTER";
            case "Confirm password" -> "CONFIRM";
            case "Type CONFIRM to acknowledge" -> "ACKNOWLEDGMENT";
            case "Title" -> "TITLE";
            case "Password" -> "PASSWORD";
            default -> "FIELD";
        };
        hideKeyboard();
        long focusDeadline = SystemClock.elapsedRealtime() + 5000;
        boolean focused = false;
        do {
            AccessibilityNodeInfo target = await(label, true, true);
            try {
                require(target.performAction(AccessibilityNodeInfo.ACTION_CLICK), "SETUP_FIELD_ACTION");
            } finally { target.recycle(); }
            SystemClock.sleep(50);
            AccessibilityNodeInfo node = find(label, true);
            if (node != null) {
                focused = node.isFocused();
                node.recycle();
            }
            if (focused) break;
            SystemClock.sleep(50);
        } while (SystemClock.elapsedRealtime() < focusDeadline);
        require(focused, "SETUP_FIELD_FOCUS");
        automation.waitForIdle(250, 5000);
        // Use actual key events. Android can cache a SET_TEXT acknowledgment
        // even before the focused Dart controller has registered its handler.
        sendStringSync(value);
        long deadline = SystemClock.elapsedRealtime() + 10_000;
        boolean updated = false;
        do {
            AccessibilityNodeInfo node = find(label, true);
            if (node != null) {
                CharSequence actual = node.getText();
                updated = actual != null && (value.contentEquals(actual)
                        || (node.isPassword() && actual.length() == value.length()));
                node.recycle();
            }
            if (updated) break;
            SystemClock.sleep(50);
        } while (SystemClock.elapsedRealtime() < deadline);
        require(updated, "SETUP_TEXT_NOT_APPLIED");
        SystemClock.sleep(100);
    }

    private void hideKeyboard() throws Exception {
        onMain(() -> {
            InputMethodManager input = (InputMethodManager) activity.getSystemService(Context.INPUT_METHOD_SERVICE);
            require(input != null, "SETUP_INPUT_SERVICE");
            input.hideSoftInputFromWindow(activity.getWindow().getDecorView().getWindowToken(), 0);
        });
        automation.waitForIdle(250, 5000);
    }

    private void tap(String label) throws Exception {
        hideKeyboard();
        automation.waitForIdle(250, 5000);
        // Tooltip semantics may wrap the button's action. Use a real touch
        // for buttons, retaining exact semantic activation for editable fields.
        AccessibilityNodeInfo node = await(label, false);
        try {
            node.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());
            automation.waitForIdle(250, 5000);
            require(node.refresh(), "SETUP_TOUCH_TARGET");
            Rect bounds = new Rect();
            node.getBoundsInScreen(bounds);
            Rect viewport = new Rect();
            onMain(() -> activity.getWindow().getDecorView().getWindowVisibleDisplayFrame(viewport));
            require(!bounds.isEmpty() && viewport.contains(bounds.centerX(), bounds.centerY()),
                    "SETUP_TOUCH_BOUNDS");
            long now = SystemClock.uptimeMillis();
            MotionEvent down = MotionEvent.obtain(now, now, MotionEvent.ACTION_DOWN,
                    bounds.exactCenterX(), bounds.exactCenterY(), 0);
            MotionEvent up = MotionEvent.obtain(now, now + 60, MotionEvent.ACTION_UP,
                    bounds.exactCenterX(), bounds.exactCenterY(), 0);
            try {
                down.setSource(InputDevice.SOURCE_TOUCHSCREEN);
                up.setSource(InputDevice.SOURCE_TOUCHSCREEN);
                require(automation.injectInputEvent(down, true)
                        && automation.injectInputEvent(up, true), "SETUP_TOUCH_INJECTION");
            } finally { down.recycle(); up.recycle(); }
        } finally { node.recycle(); }
        SystemClock.sleep(150);
    }
}
