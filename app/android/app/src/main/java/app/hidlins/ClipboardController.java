package app.hidlins;

import android.content.ClipData;
import android.content.ClipDescription;
import android.content.ClipboardManager;
import android.content.Context;
import android.os.Handler;
import android.os.Looper;
import android.os.PersistableBundle;
import java.nio.ByteBuffer;
import java.nio.CharBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.security.SecureRandom;
import java.util.Arrays;

/** Android clipboard mechanism; policy and secret selection stay in Rust. */
final class ClipboardController implements HidlinsNative.ClipboardReceiver {
    private static final String OWNER_KEY = "app.hidlins.clipboard.OWNER";
    private static final String SENSITIVE_KEY = "android.content.extra.IS_SENSITIVE";
    private static final String LABEL_PREFIX = "hidlins-secret-";

    private final ClipboardManager clipboard;
    private final Handler handler;
    private final SecureRandom random = new SecureRandom();
    private String ownerToken;
    private Runnable pendingClear;
    private int pendingTtlSeconds;

    ClipboardController(Context context) {
        clipboard = (ClipboardManager) context.getSystemService(Context.CLIPBOARD_SERVICE);
        handler = new Handler(Looper.getMainLooper());
    }

    boolean consume(String transferId, int ttlSeconds) {
        if (transferId == null || transferId.isEmpty() || ttlSeconds < 1 || ttlSeconds > 300) {
            return false;
        }
        pendingTtlSeconds = ttlSeconds;
        return HidlinsNative.consumeClipboard(transferId, this) == 0;
    }

    @Override
    public boolean receive(byte[] value) {
        return write(value, pendingTtlSeconds);
    }

    boolean write(byte[] value, int ttlSeconds) {
        if (value == null || value.length == 0 || ttlSeconds < 1 || ttlSeconds > 300) {
            if (value != null) Arrays.fill(value, (byte) 0);
            return false;
        }
        char[] chars = null;
        try {
            CharBuffer decoded = StandardCharsets.UTF_8.newDecoder()
                    .onMalformedInput(CodingErrorAction.REPORT)
                    .onUnmappableCharacter(CodingErrorAction.REPORT)
                    .decode(ByteBuffer.wrap(value));
            chars = new char[decoded.remaining()];
            decoded.get(chars);
            String token = nextToken();
            ClipData clip = ClipData.newPlainText(LABEL_PREFIX + token, CharBuffer.wrap(chars));
            PersistableBundle extras = new PersistableBundle();
            extras.putString(OWNER_KEY, token);
            extras.putBoolean(SENSITIVE_KEY, true);
            clip.getDescription().setExtras(extras);
            clipboard.setPrimaryClip(clip);
            scheduleClear(token, ttlSeconds);
            return true;
        } catch (CharacterCodingException | RuntimeException error) {
            return false;
        } finally {
            Arrays.fill(value, (byte) 0);
            if (chars != null) Arrays.fill(chars, '\0');
        }
    }

    private String nextToken() {
        byte[] bytes = new byte[16];
        random.nextBytes(bytes);
        StringBuilder token = new StringBuilder(32);
        for (byte value : bytes) token.append(String.format(java.util.Locale.ROOT, "%02x", value));
        Arrays.fill(bytes, (byte) 0);
        return token.toString();
    }

    private void scheduleClear(String token, int ttlSeconds) {
        if (pendingClear != null) handler.removeCallbacks(pendingClear);
        ownerToken = token;
        pendingClear = () -> clearIfOwned(token);
        handler.postDelayed(pendingClear, ttlSeconds * 1000L);
    }

    private void clearIfOwned(String token) {
        ClipDescription description = clipboard.getPrimaryClipDescription();
        PersistableBundle extras = description == null ? null : description.getExtras();
        String actual = extras == null ? null : extras.getString(OWNER_KEY);
        if (token.equals(ownerToken) && token.equals(actual)) clipboard.clearPrimaryClip();
        if (token.equals(ownerToken)) {
            ownerToken = null;
            pendingClear = null;
        }
    }

    void clearOwnedNowForTest() {
        String token = ownerToken;
        if (token != null) clearIfOwned(token);
    }
}
