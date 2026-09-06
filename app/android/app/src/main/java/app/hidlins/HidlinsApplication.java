package app.hidlins;

import android.app.Application;

/** Initializes the native bridge and platform TLS verifier before Flutter. */
public final class HidlinsApplication extends Application {
    private static volatile HidlinsApplication current;

    @Override
    public void onCreate() {
        super.onCreate();
        current = this;
        System.loadLibrary("hidlins_api");
        if (!HidlinsNative.initVerifier(this)) {
            throw new IllegalStateException("hidlins: TLS verifier initialization returned false");
        }
    }

    static HidlinsApplication currentForTest() {
        return current;
    }
}
