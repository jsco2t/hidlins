package app.hidlins.spike

import android.app.Activity
import android.os.Bundle
import android.util.Log
import android.widget.ScrollView
import android.widget.TextView
import app.hidlins.HidlinsNative
import kotlin.concurrent.thread

/**
 * T6.2 spike driver. Two runs prove the two claims:
 *
 * 1. Normal launch — load library, `initVerifier(context)`, then a real
 *    HTTPS HEAD through hidlins-sync's ureq/rustls stack. Expected logcat:
 *    `SPIKE_RESULT: OK: HTTP <status> — TLS handshake + platform
 *    certificate verification succeeded`.
 *
 * 2. `adb shell am start -n app.hidlins.spike/.MainActivity --ez skip_init
 *    true` (fresh process) — the negative control: the SAME probe without
 *    init must produce a clear, immediate error (never a hang), proving
 *    the missing-init failure mode is diagnosable.
 *
 * Results go to logcat under tag [TAG] (machine-checkable) and onto the
 * screen (screenshot evidence for the memo).
 */
class MainActivity : Activity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val text = TextView(this).apply { setPadding(32, 64, 32, 64); textSize = 14f }
        setContentView(ScrollView(this).apply { addView(text) })

        val skipInit = intent.getBooleanExtra("skip_init", false)
        val verifyInitFailure = intent.getBooleanExtra("verify_init_failure", false)
        val report = StringBuilder(
            "hidlins android TLS spike\nskip_init=$skipInit\n" +
                "verify_init_failure=$verifyInitFailure\n\n",
        )
        text.text = report.toString()

        // The probe blocks on network I/O — keep it off the main thread
        // (StrictMode would kill it there anyway).
        thread(name = "spike-probe") {
            fun emit(line: String) {
                Log.i(TAG, line)
                report.appendLine(line)
                runOnUiThread { text.text = report.toString() }
            }

            try {
                System.loadLibrary("hidlins_api")
                emit("loadLibrary(hidlins_api): OK")

                if (verifyInitFailure) {
                    try {
                        val unexpected = SpikeNative.probeInitFailure()
                        error("invalid-context init returned $unexpected instead of throwing")
                    } catch (e: IllegalStateException) {
                        check(e.message?.startsWith("hidlins: rustls-platform-verifier init failed:") == true) {
                            "unexpected init exception: $e"
                        }
                        emit("INIT_FAILURE_RESULT: OK: ${e::class.java.simpleName}")
                    }

                    val retryOk = HidlinsNative.initVerifier(applicationContext)
                    check(retryOk) { "valid verifier retry returned false" }
                    emit("INIT_RETRY_RESULT: OK")
                } else if (!skipInit) {
                    val ok = try {
                        HidlinsNative.initVerifier(applicationContext)
                    } catch (e: Exception) {
                        emit("initVerifier threw: $e")
                        false
                    }
                    emit("initVerifier: ${if (ok) "OK" else "FAILED"}")
                } else {
                    emit("initVerifier: SKIPPED (negative control)")
                }

                val outcome = SpikeNative.probeTls(PROBE_URL)
                emit("SPIKE_RESULT: $outcome")
            } catch (t: Throwable) {
                emit("SPIKE_RESULT: HARNESS_ERROR: $t")
            }
        }
    }

    private companion object {
        const val TAG = "HidlinsSpike"

        // Any real TLS endpoint works; s3.amazonaws.com matches the
        // product's actual sync peer class. An HTTP-level rejection (403)
        // still proves the TLS handshake + chain verification succeeded.
        const val PROBE_URL = "https://s3.amazonaws.com/"
    }
}
