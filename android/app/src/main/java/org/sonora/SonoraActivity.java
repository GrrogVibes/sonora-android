package org.sonora;

import android.content.pm.ActivityInfo;
import android.content.pm.PackageManager;
import android.media.AudioManager;
import android.os.Build;
import android.os.Bundle;
import android.util.Log;

/**
 * Hosts the Rust UI in a NativeActivity.
 *
 * The IME plumbing lives in {@link dev.gpui.mobile.GpuiInputActivity}: it owns
 * the hidden {@code EditText} that gives the keyboard an InputConnection, and
 * the native side calls its {@code gpuiShowKeyboard} method by name. The class
 * is shipped under its original package because the JNI symbols are
 * name-mangled after it.
 */
public class SonoraActivity extends dev.gpui.mobile.GpuiInputActivity {

    private static final String TAG = "SonoraActivity";

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        // Volume keys adjust the media stream, not the ringer.
        setVolumeControlStream(AudioManager.STREAM_MUSIC);
        super.onCreate(savedInstanceState);

        // The media notification is pointless without this on API 33+.
        if (Build.VERSION.SDK_INT >= 33
                && checkSelfPermission(android.Manifest.permission.POST_NOTIFICATIONS)
                        != PackageManager.PERMISSION_GRANTED) {
            requestPermissions(
                    new String[]{android.Manifest.permission.POST_NOTIFICATIONS}, 1);
        }
    }
}
