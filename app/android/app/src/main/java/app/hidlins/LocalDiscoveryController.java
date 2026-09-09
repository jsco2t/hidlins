package app.hidlins;

import android.Manifest;
import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.net.nsd.NsdManager;
import android.net.nsd.NsdServiceInfo;
import android.net.wifi.WifiManager;
import android.os.Build;
import android.os.Handler;
import android.os.Looper;
import android.provider.Settings;
import java.net.InetAddress;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import io.flutter.plugin.common.MethodChannel;

/** Android NSD mechanism adapter. Rust remains the endpoint policy authority. */
final class LocalDiscoveryController {
    static final int REQUEST_LOCAL_NETWORK = 0x484c;
    static final int MAX_CANDIDATES = 8;
    private static final String ACCESS_LOCAL_NETWORK = "android.permission.ACCESS_LOCAL_NETWORK";
    private static final String PREFS = "hidlins.local-discovery";
    private static final String ASKED = "asked";

    private final MainActivity activity;
    private final NsdManager manager;
    private final Handler handler = new Handler(Looper.getMainLooper());
    private NsdManager.DiscoveryListener listener;
    private final LinkedHashMap<String, Map<String, Object>> candidates = new LinkedHashMap<>();
    private MethodChannel.Result pending;
    private String pendingKind;
    private int pendingTimeout;
    private WifiManager.MulticastLock multicastLock;

    LocalDiscoveryController(MainActivity activity) {
        this.activity = activity;
        manager = (NsdManager) activity.getSystemService(Context.NSD_SERVICE);
    }

    Map<String, Object> permissionStatus() {
        return boundedPayload(permission(), List.of());
    }

    void discover(String kind, int timeoutMilliseconds, MethodChannel.Result result) {
        if (pending != null || !("trusted".equals(kind) || "pairing".equals(kind))
                || timeoutMilliseconds < 100 || timeoutMilliseconds > 5_000) {
            result.success(PlatformEnvelope.failure("invalid-discovery-request").envelope());
            return;
        }
        pending = result;
        pendingKind = kind;
        pendingTimeout = timeoutMilliseconds;
        if (!hasRuntimePermission()) {
            activity.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
                    .edit().putBoolean(ASKED, true).apply();
            activity.requestPermissions(requiredPermissions(), REQUEST_LOCAL_NETWORK);
            return;
        }
        startDiscovery();
    }

    void onPermissionResult(int requestCode, int[] grantResults) {
        if (requestCode != REQUEST_LOCAL_NETWORK || pending == null) return;
        boolean granted = grantResults.length > 0;
        for (int result : grantResults) granted &= result == PackageManager.PERMISSION_GRANTED;
        if (granted) startDiscovery();
        else finish("denied");
    }

    @SuppressWarnings("deprecation")
    private void startDiscovery() {
        candidates.clear();
        acquireMulticastLock();
        final String serviceType = "pairing".equals(pendingKind)
                ? "_hidlins-pair._tcp." : "_hidlins-sync._tcp.";
        listener = new NsdManager.DiscoveryListener() {
            @Override public void onDiscoveryStarted(String type) { }

            @Override public void onServiceFound(NsdServiceInfo service) {
                if (!serviceType.equals(service.getServiceType()) || candidates.size() >= MAX_CANDIDATES) {
                    return;
                }
                manager.resolveService(service, new NsdManager.ResolveListener() {
                    @Override public void onResolveFailed(NsdServiceInfo ignored, int code) { }

                    @Override public void onServiceResolved(NsdServiceInfo resolved) {
                        InetAddress host = resolved.getHost();
                        int port = resolved.getPort();
                        if (host == null || port < 1 || port > 65_535
                                || candidates.size() >= MAX_CANDIDATES) return;
                        String address = host.getHostAddress();
                        if (address == null || address.length() > 64) return;
                        int percent = address.indexOf('%');
                        int scope = 0;
                        if (percent >= 0) {
                            try {
                                scope = Integer.parseInt(address.substring(percent + 1));
                            } catch (NumberFormatException error) {
                                return;
                            }
                            address = address.substring(0, percent);
                        }
                        Map<String, Object> route = new HashMap<>();
                        route.put("address", address);
                        route.put("port", port);
                        route.put("scopeId", scope);
                        candidates.put(address + "%" + scope + ":" + port, route);
                    }
                });
            }

            @Override public void onServiceLost(NsdServiceInfo service) { }

            @Override public void onDiscoveryStopped(String type) { }

            @Override public void onStartDiscoveryFailed(String type, int code) {
                finish("restricted");
            }

            @Override public void onStopDiscoveryFailed(String type, int code) {
                finish("restricted");
            }
        };
        try {
            manager.discoverServices(serviceType, NsdManager.PROTOCOL_DNS_SD, listener);
            handler.postDelayed(() -> finish("granted"), pendingTimeout);
        } catch (SecurityException error) {
            finish("denied");
        } catch (RuntimeException error) {
            finish("restricted");
        }
    }

    void openSettings(MethodChannel.Result result) {
        Intent intent = new Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS)
                .setData(android.net.Uri.parse("package:" + activity.getPackageName()));
        try {
            activity.startActivity(intent);
            result.success(PlatformEnvelope.success(null).envelope());
        } catch (RuntimeException error) {
            result.success(PlatformEnvelope.failure("settings-unavailable").envelope());
        }
    }

    void stop() {
        handler.removeCallbacksAndMessages(null);
        if (listener != null) {
            try {
                manager.stopServiceDiscovery(listener);
            } catch (RuntimeException ignored) { }
            listener = null;
        }
        releaseMulticastLock();
        candidates.clear();
        if (pending != null) {
            MethodChannel.Result result = pending;
            pending = null;
            pendingKind = null;
            result.success(PlatformEnvelope.canceled().envelope());
        }
    }

    private void finish(String permission) {
        handler.removeCallbacksAndMessages(null);
        if (listener != null) {
            try {
                manager.stopServiceDiscovery(listener);
            } catch (RuntimeException ignored) { }
            listener = null;
        }
        releaseMulticastLock();
        if (pending == null) return;
        MethodChannel.Result result = pending;
        pending = null;
        pendingKind = null;
        result.success(PlatformEnvelope.success(
                boundedPayload(permission, new ArrayList<>(candidates.values()))).envelope());
        candidates.clear();
    }

    static Map<String, Object> boundedPayload(String permission, List<Map<String, Object>> routes) {
        Map<String, Object> value = new HashMap<>();
        value.put("permission", permission);
        List<Map<String, Object>> bounded = new ArrayList<>();
        for (Map<String, Object> route : routes) {
            if (bounded.size() >= MAX_CANDIDATES) break;
            Object rawAddress = route.get("address");
            Object rawPort = route.get("port");
            Object rawScope = route.get("scopeId");
            if (!(rawAddress instanceof String address)
                    || !(rawPort instanceof Number port)
                    || !(rawScope instanceof Number scope)
                    || address.isEmpty() || address.length() > 64
                    || port.intValue() < 1 || port.intValue() > 65_535
                    || scope.intValue() < 0) continue;
            Map<String, Object> sanitized = new HashMap<>();
            sanitized.put("address", address);
            sanitized.put("port", port.intValue());
            sanitized.put("scopeId", scope.intValue());
            bounded.add(sanitized);
        }
        value.put("candidates", bounded);
        return value;
    }

    private String permission() {
        if (hasRuntimePermission()) return "granted";
        return activity.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getBoolean(ASKED, false)
                ? "denied" : "notDetermined";
    }

    private boolean hasRuntimePermission() {
        for (String permission : requiredPermissions()) {
            if (activity.checkSelfPermission(permission) != PackageManager.PERMISSION_GRANTED) return false;
        }
        return true;
    }

    private String[] requiredPermissions() {
        List<String> permissions = new ArrayList<>();
        if (Build.VERSION.SDK_INT >= 33) permissions.add(Manifest.permission.NEARBY_WIFI_DEVICES);
        // ACCESS_LOCAL_NETWORK is enforced by Android 17 / API 37. The API 36
        // SDK accepts the forward-compatible manifest name, but API 36 devices
        // do not expose it as a grantable runtime permission.
        if (Build.VERSION.SDK_INT >= 37 && platformDefinesPermission(ACCESS_LOCAL_NETWORK)) {
            permissions.add(ACCESS_LOCAL_NETWORK);
        }
        return permissions.toArray(new String[0]);
    }

    private boolean platformDefinesPermission(String permission) {
        try {
            activity.getPackageManager().getPermissionInfo(permission, 0);
            return true;
        } catch (PackageManager.NameNotFoundException ignored) {
            // The API 36 SDK exposes this opt-in manifest name before every
            // API 36 system image declares it. An unknown platform permission
            // must not turn otherwise-supported NSD into a permanent denial.
            return false;
        }
    }

    private void acquireMulticastLock() {
        WifiManager wifi = (WifiManager) activity.getApplicationContext()
                .getSystemService(Context.WIFI_SERVICE);
        if (wifi == null) return;
        multicastLock = wifi.createMulticastLock("hidlins-local-discovery");
        multicastLock.setReferenceCounted(false);
        multicastLock.acquire();
    }

    private void releaseMulticastLock() {
        if (multicastLock != null && multicastLock.isHeld()) multicastLock.release();
        multicastLock = null;
    }
}
