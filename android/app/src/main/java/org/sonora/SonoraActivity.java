package org.sonora;

import android.content.Context;
import android.content.pm.ActivityInfo;
import android.content.pm.PackageManager;
import android.media.AudioManager;
import android.os.Build;
import android.os.Bundle;
import android.util.Log;
import android.view.KeyEvent;

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

    /** A volume-key trace line for the on-screen diagnostic. */
    static native void nativeVolumeTrace(int keyCode, int volume, int max);

    @Override
    public boolean dispatchKeyEvent(KeyEvent event) {
        int code = event.getKeyCode();
        // The volume keys are the "main" control: they move the system media
        // stream, not Sonora's own slider. Doing it here rather than trusting
        // setVolumeControlStream keeps it working whatever the window decides.
        if (event.getAction() == KeyEvent.ACTION_DOWN
                && (code == KeyEvent.KEYCODE_VOLUME_UP
                    || code == KeyEvent.KEYCODE_VOLUME_DOWN)) {
            AudioManager audio = (AudioManager) getSystemService(Context.AUDIO_SERVICE);
            if (audio != null) {
                nativeVolumeTrace(code, audio.getStreamVolume(AudioManager.STREAM_MUSIC),
                        audio.getStreamMaxVolume(AudioManager.STREAM_MUSIC));
                audio.adjustStreamVolume(AudioManager.STREAM_MUSIC,
                        code == KeyEvent.KEYCODE_VOLUME_UP
                                ? AudioManager.ADJUST_RAISE
                                : AudioManager.ADJUST_LOWER,
                        AudioManager.FLAG_SHOW_UI);
            }
            return true;
        }

        // Some headsets deliver play/pause to the focused activity instead of the
        // media session. Forward those, and let everything else reach the app.
        if (event.getAction() == KeyEvent.ACTION_DOWN) {
            switch (event.getKeyCode()) {
                case KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE:
                case KeyEvent.KEYCODE_MEDIA_PLAY:
                case KeyEvent.KEYCODE_MEDIA_PAUSE:
                case KeyEvent.KEYCODE_MEDIA_NEXT:
                case KeyEvent.KEYCODE_MEDIA_PREVIOUS:
                case KeyEvent.KEYCODE_MEDIA_STOP:
                case KeyEvent.KEYCODE_HEADSETHOOK:
                    SonoraMediaService.nativeMediaTrace(event.getKeyCode());
                    break;
                default:
                    break;
            }
            switch (event.getKeyCode()) {
                case KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE:
                case KeyEvent.KEYCODE_MEDIA_PLAY:
                case KeyEvent.KEYCODE_MEDIA_PAUSE:
                case KeyEvent.KEYCODE_HEADSETHOOK:
                    SonoraMediaService.nativeMediaKey(event.getKeyCode());
                    return true;
                default:
                    break;
            }
        }
        return super.dispatchKeyEvent(event);
    }

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
