package app.hidlins;

import java.io.ByteArrayInputStream;
import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.util.Arrays;
import java.util.concurrent.FutureTask;
import java.util.concurrent.TimeUnit;

/** Deterministic real-filesystem peer creation between observation and mkdirs. */
public final class StorageDirectoryRace {
    private static void check(boolean value, String code) {
        if (!value) throw new AssertionError(code);
    }

    private static final class PeerCreatedRoot extends File {
        private static final long serialVersionUID = 1L;
        PeerCreatedRoot(File parent) { super(parent, "peer-created-root"); }
        @Override public boolean mkdirs() {
            FutureTask<Boolean> peer = new FutureTask<>(() -> new File(getPath()).mkdirs());
            Thread creator = new Thread(peer, "fixture-directory-peer");
            creator.setDaemon(true);
            creator.start();
            try {
                check(peer.get(5, TimeUnit.SECONDS), "fixture-peer-did-not-create-directory");
            } catch (Exception error) {
                throw new AssertionError("fixture-peer-creation-failed", error);
            }
            // This is a real false return after a real concurrent creator wins.
            return super.mkdirs();
        }
    }

    private static void invalidDirectoriesStillFail(File parent) throws IOException {
        File badRoot = new File(parent, "not-a-directory");
        Files.write(badRoot.toPath(), new byte[]{0});
        boolean refused = false;
        try { new AppStorage(null, badRoot).prepare(); }
        catch (IOException expected) { refused = true; }
        check(refused, "regular-file-root-must-fail");
        File root = new File(parent, "blocked-vaults");
        check(root.mkdir(), "fixture-root-create-failed");
        Files.write(new File(root, "Vaults").toPath(), new byte[]{0});
        refused = false;
        try { new AppStorage(null, root).importVault(new ByteArrayInputStream(new byte[]{1}), "fixture.kdbx"); }
        catch (IOException expected) { refused = true; }
        check(refused, "regular-file-vaults-must-fail");
        System.out.println("PASS invalid-root-and-vaults-remain-rejected");
    }

    private static void peerCreationPreservesImport(File parent) throws IOException {
        PeerCreatedRoot root = new PeerCreatedRoot(parent);
        check(!root.exists(), "fixture-root-must-start-absent");
        byte[] bytes = new byte[]{7, 11, 19};
        File imported = new AppStorage(null, root).importVault(new ByteArrayInputStream(bytes), "fixture.kdbx");
        check(root.isDirectory(), "peer-created-root-unavailable");
        check(imported.getName().equals("fixture.kdbx"), "import-name-changed");
        check(Arrays.equals(bytes, Files.readAllBytes(imported.toPath())), "import-bytes-changed");
        check(new File(root, "Vaults").equals(imported.getParentFile()), "import-left-owned-directory");
        System.out.println("PASS peer-directory-creation-preserves-import");
    }

    private static void remove(File file) throws IOException {
        File[] children = file.listFiles();
        if (children != null) for (File child : children) remove(child);
        if (!file.delete()) throw new IOException("fixture-cleanup-failed");
    }

    public static void main(String[] args) throws IOException {
        File parent = Files.createTempDirectory("hidlins-directory-race-").toFile();
        try {
            invalidDirectoriesStillFail(parent);
            peerCreationPreservesImport(parent);
        } finally {
            remove(parent);
        }
    }
}
