package app.hidlins;

import android.content.Context;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.util.UUID;

/** App-owned state and atomic provider-to-vault copy mechanism. */
final class AppStorage {
    private final File root;

    AppStorage(Context context) {
        this(context, new File(context.getFilesDir(), "hidlins"));
    }

    AppStorage(Context context, File root) {
        this.root = root;
    }

    File prepare() throws IOException {
        if (!root.isDirectory() && !root.mkdirs() && !root.isDirectory()) {
            throw new IOException("app support unavailable");
        }
        return root;
    }

    File importVault(InputStream source, String displayName) throws IOException {
        File temporary = null;
        boolean installed = false;
        try {
            File vaults;
            String safe;
            // Own the source before setup, and close both streams before
            // installation so a close failure cannot orphan an installed file.
            try (InputStream input = source) {
                vaults = new File(prepare(), "Vaults");
                if (!vaults.isDirectory() && !vaults.mkdirs() && !vaults.isDirectory()) {
                    throw new IOException("vault directory unavailable");
                }
                safe = safeVaultName(displayName);
                temporary = new File(vaults, ".import-" + UUID.randomUUID());
                try (FileOutputStream output = new FileOutputStream(temporary)) {
                    byte[] buffer = new byte[16 * 1024];
                    try {
                        for (int count; (count = input.read(buffer)) != -1; ) output.write(buffer, 0, count);
                    } finally {
                        java.util.Arrays.fill(buffer, (byte) 0);
                    }
                    output.getFD().sync();
                }
            }
            // Copies overlap across activity/storage instances. Serialize only
            // name selection and atomic rename, never provider I/O or copying.
            synchronized (AppStorage.class) {
                File destination = uniqueDestination(vaults, safe);
                if (!temporary.renameTo(destination)) throw new IOException("atomic vault install failed");
                installed = true;
                return destination;
            }
        } finally {
            if (!installed && temporary != null && temporary.exists() && !temporary.delete()) temporary.deleteOnExit();
        }
    }

    private static String safeVaultName(String supplied) throws IOException {
        String name = supplied == null ? "" : supplied.replaceAll("[^a-zA-Z0-9._ -]", "-");
        if (name.endsWith(".kdbx")) name = name.substring(0, name.length() - 5);
        name = name.replaceAll("^\\.+", "").trim();
        if (name.isEmpty()) throw new IOException("invalid vault name");
        return name + ".kdbx";
    }

    private static File uniqueDestination(File directory, String name) {
        File first = new File(directory, name);
        if (!first.exists()) return first;
        String base = name.substring(0, name.length() - 5);
        return new File(directory, base + "-" + UUID.randomUUID() + ".kdbx");
    }
}
