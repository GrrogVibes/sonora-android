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
        // stream, not Sonora's own slider. Done here as well as through
        // setVolumeControlStream, because a NativeActivity window does not
        // always get them.
        if (event.getAction() == KeyEvent.ACTION_DOWN
                && (code == KeyEvent.KEYCODE_VOLUME_UP
                    || code == KeyEvent.KEYCODE_VOLUME_DOWN)) {
            adjustVolume(code);
            return true;
        }

        // Some headsets deliver play/pause to the focused activity instead of the
        // media session. Forward those, and let everything else reach the app.
        if (event.getAction() == KeyEvent.ACTION_DOWN) {
            switch (code) {
                case KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE:
                case KeyEvent.KEYCODE_MEDIA_PLAY:
                case KeyEvent.KEYCODE_MEDIA_PAUSE:
                case KeyEvent.KEYCODE_MEDIA_NEXT:
                case KeyEvent.KEYCODE_MEDIA_PREVIOUS:
                case KeyEvent.KEYCODE_MEDIA_STOP:
                case KeyEvent.KEYCODE_HEADSETHOOK:
                    SonoraMediaService.nativeMediaTrace(code);
                    break;
                default:
                    break;
            }
            switch (code) {
                case KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE:
                case KeyEvent.KEYCODE_MEDIA_PLAY:
                case KeyEvent.KEYCODE_MEDIA_PAUSE:
                case KeyEvent.KEYCODE_HEADSETHOOK:
                    SonoraMediaService.nativeMediaKey(code);
                    return true;
                default:
                    break;
            }
        }
        return super.dispatchKeyEvent(event);
    }

    @Override
    public boolean onKeyDown(int keyCode, KeyEvent event) {
        if (keyKeyIsVolume(keyCode)) {
            adjustVolume(keyCode);
            return true;
        }
        return super.onKeyDown(keyCode, event);
    }

    @Override
    public boolean onKeyUp(int keyCode, KeyEvent event) {
        if (keyKeyIsVolume(keyCode)) {
            return true;
        }
        return super.onKeyUp(keyCode, event);
    }

    private static boolean keyKeyIsVolume(int keyCode) {
        return keyCode == KeyEvent.KEYCODE_VOLUME_UP
                || keyCode == KeyEvent.KEYCODE_VOLUME_DOWN;
    }

    /** Moves the system media stream and leaves a trace of what it did. */
    private void adjustVolume(int keyCode) {
        AudioManager audio = (AudioManager) getSystemService(Context.AUDIO_SERVICE);
        if (audio == null) {
            return;
        }
        nativeVolumeTrace(keyCode, audio.getStreamVolume(AudioManager.STREAM_MUSIC),
                audio.getStreamMaxVolume(AudioManager.STREAM_MUSIC));
        audio.adjustStreamVolume(AudioManager.STREAM_MUSIC,
                keyCode == KeyEvent.KEYCODE_VOLUME_UP
                        ? AudioManager.ADJUST_RAISE
                        : AudioManager.ADJUST_LOWER,
                AudioManager.FLAG_SHOW_UI);
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        // Volume keys adjust the media stream, not the ringer. Done after
        // super.onCreate so the window it is tied to already exists.
        setVolumeControlStream(AudioManager.STREAM_MUSIC);
        getWindow().setVolumeControlStream(AudioManager.STREAM_MUSIC);

        // The media notification is pointless without this on API 33+.
        if (Build.VERSION.SDK_INT >= 33
                && checkSelfPermission(android.Manifest.permission.POST_NOTIFICATIONS)
                        != PackageManager.PERMISSION_GRANTED) {
            requestPermissions(
                    new String[]{android.Manifest.permission.POST_NOTIFICATIONS}, 1);
        }
    }
}
