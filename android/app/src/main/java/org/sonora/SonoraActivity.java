package org.sonora;

import android.content.pm.PackageManager;
import android.media.AudioManager;
import android.os.Build;
import android.os.Bundle;
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

    @Override
    public boolean dispatchKeyEvent(KeyEvent event) {
        // The volume keys are the system's "main" control and are adjusted by
        // gpui-mobile's volume_key_android, which sees the native input queue a
        // NativeActivity feeds. Intercepting them here as well would step the
        // volume twice.
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
        super.onCreate(savedInstanceState);
        // The volume keys belong to the media stream, not the ringer. The
        // stream itself is moved on the native side, which is where a
        // NativeActivity's key events actually arrive.
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
