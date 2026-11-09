package app.hidlins;

import android.content.ContentProvider;
import android.content.ContentValues;
import android.content.Intent;
import android.database.Cursor;
import android.database.MatrixCursor;
import android.net.Uri;
import android.os.Bundle;
import android.os.CancellationSignal;
import android.os.OperationCanceledException;
import android.os.ParcelFileDescriptor;
import android.provider.OpenableColumns;
import android.system.Os;
import android.system.OsConstants;
import android.system.StructPollfd;
import android.system.ErrnoException;
import java.io.FileNotFoundException;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;

/** Test-APK provider: Binder-controlled gates never depend on cross-process static latches. */
public final class HidlinsControlledDocumentProvider extends ContentProvider {
    static final String AUTHORITY = "app.hidlins.test.controlled-document";
    static final byte[] CONTENT = "native-provider-ciphertext-fixture".getBytes(StandardCharsets.UTF_8);
    private final ConcurrentHashMap<String, Document> documents = new ConcurrentHashMap<>();

    private static final class Document {
        final String phase;
        final boolean honorCancellation;
        final boolean observeReaderClose;
        final AtomicBoolean readerClosed = new AtomicBoolean();
        final AtomicBoolean readerCloseTimedOut = new AtomicBoolean();
        final CountDownLatch entered = new CountDownLatch(1);
        final CountDownLatch released = new CountDownLatch(1);
        final AtomicBoolean canceled = new AtomicBoolean();
        final AtomicBoolean timedOut = new AtomicBoolean();
        final AtomicBoolean writerClosed = new AtomicBoolean();

        Document(Bundle arguments) {
            phase = arguments.getString("phase", "none");
            if (!phase.equals("none") && !phase.equals("query")
                    && !phase.equals("open") && !phase.equals("read")) {
                throw new IllegalArgumentException("unknown fixture phase");
            }
            honorCancellation = arguments.getBoolean("honorCancellation", false);
            observeReaderClose = arguments.getBoolean("observeReaderClose", false);
        }

        void observe(CancellationSignal signal) {
            if (signal != null) signal.setOnCancelListener(() -> canceled.set(true));
        }

        void gate(String current) {
            if (!phase.equals(current)) return;
            entered.countDown();
            long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(15);
            for (;;) {
                if (honorCancellation && canceled.get()) throw new OperationCanceledException();
                try {
                    if (released.await(20, TimeUnit.MILLISECONDS)) return;
                } catch (InterruptedException ignored) {
                    // The stubborn-provider case proves destruction never waits
                    // for an external provider to honor thread interruption.
                }
                if (System.nanoTime() >= deadline) {
                    timedOut.set(true);
                    throw new IllegalStateException("fixture gate timed out");
                }
            }
        }
    }

    @Override public boolean onCreate() { return true; }
    @Override public String getType(Uri uri) { return "application/octet-stream"; }

    private Document document(Uri uri) {
        Document document = documents.get(uri.getLastPathSegment());
        if (document == null) throw new IllegalArgumentException("unknown fixture document");
        return document;
    }

    @Override public Bundle call(String method, String argument, Bundle extras) {
        if (argument == null || argument.isEmpty()) throw new IllegalArgumentException("missing fixture id");
        Bundle result = new Bundle();
        if (method.equals("configure")) {
            if (extras == null || documents.putIfAbsent(argument, new Document(extras)) != null) {
                throw new IllegalArgumentException("invalid or duplicate fixture");
            }
            result.putBoolean("configured", true);
            return result;
        }
        Document document = documents.get(argument);
        if (document == null) throw new IllegalArgumentException("unknown fixture id");
        switch (method) {
            case "release" -> document.released.countDown();
            case "remove" -> {
                document.released.countDown();
                documents.remove(argument, document);
            }
            case "grant" -> {
                if (getContext() == null) throw new IllegalStateException("missing provider context");
                getContext().grantUriPermission("app.hidlins",
                        new Uri.Builder().scheme("content").authority(AUTHORITY).appendPath(argument).build(),
                        Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION);
            }
            case "state" -> { }
            default -> throw new IllegalArgumentException("unknown fixture command");
        }
        result.putBoolean("entered", document.entered.getCount() == 0);
        result.putBoolean("released", document.released.getCount() == 0);
        result.putBoolean("canceled", document.canceled.get());
        result.putBoolean("timedOut", document.timedOut.get());
        result.putBoolean("writerClosed", document.writerClosed.get());
        result.putBoolean("readerClosed", document.readerClosed.get());
        result.putBoolean("readerCloseTimedOut", document.readerCloseTimedOut.get());
        return result;
    }

    @Override public Cursor query(Uri uri, String[] projection, String selection,
                                   String[] selectionArgs, String sortOrder) {
        return query(uri, projection, selection, selectionArgs, sortOrder, null);
    }

    @Override public Cursor query(Uri uri, String[] projection, String selection,
                                   String[] selectionArgs, String sortOrder, CancellationSignal signal) {
        Document document = document(uri);
        document.observe(signal);
        document.gate("query");
        MatrixCursor cursor = new MatrixCursor(new String[]{OpenableColumns.DISPLAY_NAME});
        cursor.addRow(new Object[]{"controlled-provider.kdbx"});
        return cursor;
    }

    @Override public ParcelFileDescriptor openFile(Uri uri, String mode) throws FileNotFoundException {
        return openFile(uri, mode, null);
    }

    @Override public ParcelFileDescriptor openFile(Uri uri, String mode, CancellationSignal signal)
            throws FileNotFoundException {
        if (!mode.equals("r")) throw new FileNotFoundException("read-only fixture");
        Document document = document(uri);
        document.observe(signal);
        document.gate("open");
        try {
            ParcelFileDescriptor[] pipe = ParcelFileDescriptor.createPipe();
            Thread writer = new Thread(() -> {
                try (ParcelFileDescriptor.AutoCloseOutputStream output =
                        new ParcelFileDescriptor.AutoCloseOutputStream(pipe[1])) {
                    try {
                        output.write(CONTENT);
                        document.gate("read");
                    } finally {
                        if (document.observeReaderClose) {
                            // POLLERR on a pipe writer is kernel evidence that
                            // every reader endpoint has closed, not writer EOF.
                            StructPollfd peer = new StructPollfd();
                            peer.fd = pipe[1].getFileDescriptor();
                            peer.events = (short) (OsConstants.POLLERR | OsConstants.POLLHUP);
                            long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(1);
                            while (System.nanoTime() < deadline) {
                                Os.poll(new StructPollfd[]{peer}, 20);
                                if ((peer.revents & OsConstants.POLLERR) != 0) {
                                    document.readerClosed.set(true);
                                    break;
                                }
                            }
                            if (!document.readerClosed.get()) document.readerCloseTimedOut.set(true);
                        }
                    }
                } catch (IOException | ErrnoException | RuntimeException ignored) {
                    // Cancellation can close the app's reader. State remains
                    // observable through Binder without logging any contents.
                } finally {
                    document.writerClosed.set(true);
                }
            }, "hidlins-provider-fixture");
            writer.setDaemon(true);
            writer.start();
            return pipe[0];
        } catch (IOException error) {
            throw new FileNotFoundException("fixture pipe unavailable");
        }
    }

    @Override public Uri insert(Uri uri, ContentValues values) { throw new UnsupportedOperationException(); }
    @Override public int delete(Uri uri, String selection, String[] selectionArgs) { throw new UnsupportedOperationException(); }
    @Override public int update(Uri uri, ContentValues values, String selection, String[] selectionArgs) { throw new UnsupportedOperationException(); }
}
