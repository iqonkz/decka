package app.decka.client;

import android.content.Intent;
import android.net.Uri;
import android.os.Build;
import android.provider.Settings;
import android.util.Base64;

import androidx.core.content.FileProvider;

import com.getcapacitor.JSObject;
import com.getcapacitor.Plugin;
import com.getcapacitor.PluginCall;
import com.getcapacitor.PluginMethod;
import com.getcapacitor.annotation.CapacitorPlugin;

import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.security.MessageDigest;
import java.util.Locale;

/**
 * Installs the new app that the PC handed over (T20.4). JS has already checked the SHA-256 of the whole file; here the file is
 * stored in the cache, its SHA-256 is checked once more on disk and Android's installer is opened through FileProvider
 * (ACTION_VIEW, application/vnd.android.package-archive). The first time Android asks to allow installing from this source.
 */
@CapacitorPlugin(name = "DeckaInstall")
public class DeckaInstallPlugin extends Plugin {
    private FileOutputStream out;

    private File apk() {
        File dir = new File(getContext().getCacheDir(), "updates");
        if (!dir.exists()) dir.mkdirs();
        return new File(dir, "decka-update.apk");
    }

    private void closeQuietly() {
        if (out == null) return;
        try {
            out.close();
        } catch (IOException ignored) {
            // nothing to do: the file is checked before it is used
        }
        out = null;
    }

    @PluginMethod
    public void begin(PluginCall call) {
        closeQuietly();
        try {
            File file = apk();
            if (file.exists()) file.delete();
            out = new FileOutputStream(file);
            call.resolve();
        } catch (IOException e) {
            call.reject("IO");
        }
    }

    @PluginMethod
    public void append(PluginCall call) {
        String data = call.getString("data");
        if (out == null || data == null) {
            call.reject("NOT_STARTED");
            return;
        }
        try {
            out.write(Base64.decode(data, Base64.DEFAULT));
            call.resolve();
        } catch (IOException | IllegalArgumentException e) {
            closeQuietly();
            call.reject("IO");
        }
    }

    static String sha256Hex(File file) throws Exception {
        MessageDigest digest = MessageDigest.getInstance("SHA-256");
        try (FileInputStream in = new FileInputStream(file)) {
            byte[] buf = new byte[64 * 1024];
            int n;
            while ((n = in.read(buf)) > 0) digest.update(buf, 0, n);
        }
        StringBuilder hex = new StringBuilder();
        for (byte b : digest.digest()) hex.append(String.format(Locale.ROOT, "%02x", b));
        return hex.toString();
    }

    @PluginMethod
    public void install(PluginCall call) {
        String expected = call.getString("sha256");
        closeQuietly();
        File file = apk();
        try {
            if (expected == null || !file.exists() || !sha256Hex(file).equalsIgnoreCase(expected)) {
                file.delete();
                call.reject("HASH_MISMATCH");
                return;
            }
        } catch (Exception e) {
            call.reject("IO");
            return;
        }
        // Android 8+: the user must allow this app to install packages; the system page opens, the user comes back and taps again.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O && !getContext().getPackageManager().canRequestPackageInstalls()) {
            Intent settings = new Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:" + getContext().getPackageName()));
            settings.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
            getContext().startActivity(settings);
            call.reject("NEED_PERMISSION");
            return;
        }
        Uri uri = FileProvider.getUriForFile(getContext(), getContext().getPackageName() + ".fileprovider", file);
        Intent view = new Intent(Intent.ACTION_VIEW);
        view.setDataAndType(uri, "application/vnd.android.package-archive");
        view.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_ACTIVITY_NEW_TASK);
        getContext().startActivity(view);
        call.resolve(new JSObject());
    }
}
