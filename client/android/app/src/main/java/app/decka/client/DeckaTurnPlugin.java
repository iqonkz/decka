package app.decka.client;

import android.content.pm.ActivityInfo;

import com.getcapacitor.Plugin;
import com.getcapacitor.PluginCall;
import com.getcapacitor.PluginMethod;
import com.getcapacitor.annotation.CapacitorPlugin;

/**
 * Sensor-driven orientation for the `Auto` turn (DATA_MODEL.md 10.9): the screen follows the sensor between the two sides of
 * the layout's orientation, even when Android auto-rotate is off. The stock screen-orientation plugin only offers a fixed side.
 */
@CapacitorPlugin(name = "DeckaTurn")
public class DeckaTurnPlugin extends Plugin {
    @PluginMethod
    public void setSensor(PluginCall call) {
        final boolean portrait = Boolean.TRUE.equals(call.getBoolean("portrait", false));
        getActivity().runOnUiThread(() -> {
            getActivity().setRequestedOrientation(
                portrait ? ActivityInfo.SCREEN_ORIENTATION_SENSOR_PORTRAIT : ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
            );
            call.resolve();
        });
    }
}
