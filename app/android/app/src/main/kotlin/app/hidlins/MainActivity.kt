package app.hidlins

import android.content.Intent
import android.os.Bundle
import android.view.WindowManager
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine

class MainActivity : FlutterActivity() {
    companion object {
        @Volatile
        private var current: MainActivity? = null

        @JvmStatic
        fun currentForTest(): MainActivity? = current
    }

    private val securityCover = SnapshotCoverController()
    private var platformServices: HidlinsPlatformServices? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
        super.onCreate(savedInstanceState)
        current = this
        securityCover.show(this)
    }

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        platformServices = HidlinsPlatformServices(this, securityCover).also {
            it.register(flutterEngine.dartExecutor.binaryMessenger)
        }
    }

    override fun onResume() {
        super.onResume()
        platformServices?.reportLifecycle("resumed")
    }

    override fun onPause() {
        securityCover.show(this)
        platformServices?.reportLifecycle("paused")
        super.onPause()
    }

    override fun onStop() {
        securityCover.show(this)
        platformServices?.reportLifecycle("hidden")
        super.onStop()
    }

    override fun onDestroy() {
        securityCover.show(this)
        platformServices?.reportLifecycle("detached")
        platformServices?.close()
        platformServices = null
        if (current === this) current = null
        super.onDestroy()
    }

    @Suppress("DEPRECATION")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        if (platformServices?.onActivityResult(requestCode, resultCode, data) != true) {
            super.onActivityResult(requestCode, resultCode, data)
        }
    }

    fun showSecurityCoverForTest() = securityCover.show(this)

    fun isSecurityCoverVisibleForTest(): Boolean = securityCover.isVisible
}
