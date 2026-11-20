package org.sonora;

import android.app.NativeActivity;
import android.content.pm.ActivityInfo;
import android.content.pm.PackageManager;
import android.media.AudioManager;
import android.os.Bundle;
import android.util.Log;

/**
 * Hosts the Rust UI in a NativeActivity.
 *
 * NativeActivity dlopens the .so itself, which does not register JNI symbols
 * with the classloader, so the library is loaded again explicitly to make
 * JNI-based helpers (media session, webview, permissions) resolvable.
 */
public class SonoraActivity extends NativeActivity {

    private static final String TAG = "SonoraActivity";
    private static volatile boolean sNativeLibLoaded = false;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        if (!sNativeLibLoaded) {
            try {
                ActivityInfo info = getPackageManager().getActivityInfo(
                        getComponentName(), PackageManager.GET_META_DATA);
                String libName = info.metaData.getString("android.app.lib_name");
                if (libName != null) {
                    System.loadLibrary(libName);
                    sNativeLibLoaded = true;
                }
            } catch (PackageManager.NameNotFoundException e) {
                Log.w(TAG, "no meta-data on the activity", e);
            } catch (UnsatisfiedLinkError e) {
                // Already loaded by NativeActivity; nothing to do.
                sNativeLibLoaded = true;
            }
        }

        // Volume keys adjust the media stream, not the ringer.
        setVolumeControlStream(AudioManager.STREAM_MUSIC);

        super.onCreate(savedInstanceState);
    }
}
