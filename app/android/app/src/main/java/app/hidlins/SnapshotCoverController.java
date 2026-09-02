package app.hidlins;

import android.graphics.Color;
import android.view.View;
import android.view.ViewGroup;
import android.widget.FrameLayout;

/** Opaque non-secret view installed synchronously before Android snapshots the activity. */
final class SnapshotCoverController {
    private static final String TAG = "hidlins.snapshot-cover";
    private View cover;

    void show(MainActivity activity) {
        ViewGroup decor = (ViewGroup) activity.getWindow().getDecorView();
        if (cover != null && cover.getParent() == decor) {
            cover.bringToFront();
            return;
        }
        hide();
        View view = new View(activity);
        view.setTag(TAG);
        view.setBackgroundColor(Color.rgb(250, 248, 245));
        view.setContentDescription("Hidlins is locked");
        view.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_YES);
        decor.addView(view, new FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));
        view.bringToFront();
        cover = view;
    }

    void hide() {
        if (cover != null && cover.getParent() instanceof ViewGroup parent) parent.removeView(cover);
        cover = null;
    }

    boolean isVisible() { return cover != null && cover.getParent() != null; }
}
