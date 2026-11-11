package app.hidlins;

import android.app.Activity;
import android.content.Context;
import android.content.Intent;
import android.net.nsd.NsdManager;
import android.net.nsd.NsdServiceInfo;
import android.os.Bundle;
import android.util.Log;
import java.util.UUID;

/** Test-APK-only DNS-SD publisher for the real scenario CLI listener. */
public final class ScenarioNsdRegistrarActivity extends Activity {
    static final String COMPONENT =
            "app.hidlins.test/app.hidlins.ScenarioNsdRegistrarActivity";
    static final String STOP_ACTION = "app.hidlins.test.STOP_NSD_REGISTRAR";
    static final String READY_MARKER = "HIDLINS_NSD_REGISTRAR_READY";
    static final String STOPPED_MARKER = "HIDLINS_NSD_REGISTRAR_STOPPED";
    static final String ERROR_MARKER = "HIDLINS_NSD_REGISTRAR_ERROR";
    static final String TAG = "HidlinsNsdRegistrar";

    private NsdManager manager;
    private NsdManager.RegistrationListener listener;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        manager = (NsdManager) getSystemService(Context.NSD_SERVICE);
        handle(getIntent());
    }

    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
        handle(intent);
    }

    @Override
    protected void onDestroy() {
        unregister(false);
        super.onDestroy();
    }

    @SuppressWarnings("deprecation")
    @Override
    public void onBackPressed() {
        unregister(true);
    }

    static boolean validPort(int port) {
        return port >= 1 && port <= 65_535;
    }

    static String serviceType(String kind) {
        if ("pairing".equals(kind)) return "_hidlins-pair._tcp.";
        if ("trusted".equals(kind)) return "_hidlins-sync._tcp.";
        return null;
    }

    private void handle(Intent intent) {
        if (intent != null && STOP_ACTION.equals(intent.getAction())) {
            unregister(true);
            return;
        }
        if (intent == null || listener != null) {
            failAndFinish("invalid-intent");
            return;
        }
        int port = intent.getIntExtra("port", 0);
        String kind = intent.getStringExtra("kind");
        String type = serviceType(kind);
        if (!validPort(port) || type == null || manager == null) {
            failAndFinish("invalid-registration");
            return;
        }

        NsdServiceInfo service = new NsdServiceInfo();
        // A fresh emulator commonly reuses the registrar's Linux PID. Reusing
        // that value as the instance label leaves the old DNS-SD identity in
        // peer caches after authority replacement, so every registration gets
        // a new non-sensitive instance identity instead.
        service.setServiceName("hidlins-test-" + UUID.randomUUID());
        service.setServiceType(type);
        service.setPort(port);
        service.setAttribute("v", "1");

        listener = new NsdManager.RegistrationListener() {
            @Override
            public void onRegistrationFailed(NsdServiceInfo ignored, int errorCode) {
                Log.e(TAG, ERROR_MARKER + "=registration-failed-" + errorCode);
                listener = null;
                finishAndRemoveTask();
            }

            @Override
            public void onServiceRegistered(NsdServiceInfo registered) {
                // Android may omit the port from the callback object even
                // though it retained the port from the registration request.
                Log.i(TAG, READY_MARKER + "=" + kind + ":" + port);
            }

            @Override
            public void onServiceUnregistered(NsdServiceInfo ignored) {
                Log.i(TAG, STOPPED_MARKER);
                finishAndRemoveTask();
            }

            @Override
            public void onUnregistrationFailed(NsdServiceInfo ignored, int errorCode) {
                Log.e(TAG, ERROR_MARKER + "=unregistration-failed-" + errorCode);
                finishAndRemoveTask();
            }
        };
        try {
            manager.registerService(service, NsdManager.PROTOCOL_DNS_SD, listener);
        } catch (RuntimeException error) {
            Log.e(TAG, ERROR_MARKER + "=registration-exception");
            listener = null;
            finishAndRemoveTask();
        }
    }

    private void unregister(boolean finishWhenIdle) {
        NsdManager.RegistrationListener active = listener;
        listener = null;
        if (manager != null && active != null) {
            try {
                manager.unregisterService(active);
                return;
            } catch (RuntimeException error) {
                Log.e(TAG, ERROR_MARKER + "=unregistration-exception");
            }
        }
        if (finishWhenIdle) finishAndRemoveTask();
    }

    private void failAndFinish(String code) {
        Log.e(TAG, ERROR_MARKER + "=" + code);
        finishAndRemoveTask();
    }
}
