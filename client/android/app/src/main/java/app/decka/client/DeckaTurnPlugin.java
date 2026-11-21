package app.decka.client;

import android.content.pm.ActivityInfo;
import android.os.Handler;
import android.os.Looper;
import android.view.OrientationEventListener;

import com.getcapacitor.Plugin;
import com.getcapacitor.PluginCall;
import com.getcapacitor.PluginMethod;
import com.getcapacitor.annotation.CapacitorPlugin;

/**
 * Own flip for the `Auto` turn (DATA_MODEL.md 10.9, T20.1). The system sensor modes do not flip the screen on some firmware,
 * so an OrientationEventListener picks a hard side. The layout orientation (portrait/landscape) is never changed here.
 * The sector table is the same as `turnSide` in client/src/platform/turn.ts.
 */
@CapacitorPlugin(name = "DeckaTurn")
public class DeckaTurnPlugin extends Plugin {
    private static final long HOLD_MS = 600;

    private final Handler handler = new Handler(Looper.getMainLooper());
    private OrientationEventListener listener;
    private boolean auto = false;
    private boolean portrait = false;
    private boolean secondary = false;
    private int candidate = -1; // -1 none, 0 primary, 1 secondary
    private final Runnable commit = () -> {
        if (candidate < 0) return;
        secondary = candidate == 1;
        candidate = -1;
        applySide();
    };

    /** Returns 0 primary, 1 secondary, -1 outside the sectors. */
    static int sector(int angle, boolean portrait) {
        if (angle < 0 || angle > 359) return -1;
        if (portrait) {
            if (angle >= 330 || angle <= 30) return 0;
            if (angle >= 150 && angle <= 210) return 1;
        } else {
            if (angle >= 240 && angle <= 300) return 0;
            if (angle >= 60 && angle <= 120) return 1;
        }
        return -1;
    }

    private void applySide() {
        int value;
        if (portrait) {
            value = secondary ? ActivityInfo.SCREEN_ORIENTATION_REVERSE_PORTRAIT : ActivityInfo.SCREEN_ORIENTATION_PORTRAIT;
        } else {
            value = secondary ? ActivityInfo.SCREEN_ORIENTATION_REVERSE_LANDSCAPE : ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE;
        }
        getActivity().setRequestedOrientation(value);
    }

    private void onAngle(int angle) {
        int s = sector(angle, portrait);
        if (s < 0 || s == (secondary ? 1 : 0)) {
            candidate = -1;
            handler.removeCallbacks(commit);
            return;
        }
        if (s != candidate) {
            candidate = s;
            handler.removeCallbacks(commit);
            handler.postDelayed(commit, HOLD_MS);
        }
    }

    private void startListening() {
        if (listener == null) {
            listener = new OrientationEventListener(getContext()) {
                @Override
                public void onOrientationChanged(int angle) {
                    onAngle(angle);
                }
            };
        }
        if (listener.canDetectOrientation()) listener.enable();
    }

    private void stopListening() {
        handler.removeCallbacks(commit);
        candidate = -1;
        if (listener != null) listener.disable();
    }

    @PluginMethod
    public void setAuto(PluginCall call) {
        final boolean wantPortrait = Boolean.TRUE.equals(call.getBoolean("portrait", false));
        getActivity().runOnUiThread(() -> {
            if (!auto || portrait != wantPortrait) secondary = false;
            portrait = wantPortrait;
            auto = true;
            applySide();
            startListening();
            call.resolve();
        });
    }

    @PluginMethod
    public void stop(PluginCall call) {
        getActivity().runOnUiThread(() -> {
            auto = false;
            stopListening();
            call.resolve();
        });
    }

    @Override
    protected void handleOnPause() {
        stopListening();
        super.handleOnPause();
    }

    @Override
    protected void handleOnResume() {
        super.handleOnResume();
        if (auto) startListening();
    }
}
