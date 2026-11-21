package org.sonora;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Intent;
import android.content.pm.ServiceInfo;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.os.Build;
import android.os.IBinder;
import android.support.v4.media.MediaMetadataCompat;
import android.support.v4.media.session.MediaSessionCompat;
import android.support.v4.media.session.PlaybackStateCompat;
import android.view.KeyEvent;
import androidx.core.app.NotificationCompat;

import java.net.URL;

/**
 * The always-on media notification. A foreground service owns a
 * MediaSessionCompat; the Rust side pushes track metadata at it and the media
 * buttons come back as {@code nativeMediaAction}/{@code nativeMediaSeek} calls.
 *
 * The notification buttons drive the same service through their own actions,
 * which keeps working even when the Rust side has not been asked for metadata
 * again yet.
 */
public class SonoraMediaService extends Service {

    public static final String CHANNEL = "org.sonora.media";
    private static final int NOTIFICATION_ID = 1;

    public static final String ACTION_START = "org.sonora.media.START";
    public static final String ACTION_STOP = "org.sonora.media.STOP";
    public static final String ACTION_PLAY = "org.sonora.media.PLAY";
    public static final String ACTION_PAUSE = "org.sonora.media.PAUSE";
    public static final String ACTION_NEXT = "org.sonora.media.NEXT";
    public static final String ACTION_PREVIOUS = "org.sonora.media.PREVIOUS";

    /// The actions Rust is told about; the values match `Command` in
    /// crates/state/src/remote.rs.
    public static final int MEDIA_PLAY = 1;
    public static final int MEDIA_PAUSE = 2;
    public static final int MEDIA_TOGGLE = 3;
    public static final int MEDIA_NEXT = 4;
    public static final int MEDIA_PREVIOUS = 5;

    private static SonoraMediaService current;

    private MediaSessionCompat session;
    private String title = "";
    private String artist = "";
    private String album = "";
    private String cover = "";
    private Bitmap coverBitmap;
    private boolean playing;
    private long position;
    private long duration;

    @Override
    public void onCreate() {
        super.onCreate();
        current = this;

        NotificationManager manager = (NotificationManager) getSystemService(NOTIFICATION_SERVICE);
        manager.createNotificationChannel(new NotificationChannel(
                CHANNEL, "Playback", NotificationManager.IMPORTANCE_LOW));

        session = new MediaSessionCompat(this, "Sonora");
        // Without these flags the session ignores media-button keys, which is
        // exactly how next/previous keep working while play/pause goes nowhere.
        session.setFlags(MediaSessionCompat.FLAG_HANDLES_MEDIA_BUTTONS
                | MediaSessionCompat.FLAG_HANDLES_TRANSPORT_CONTROLS);
        // Some stacks only deliver media buttons to a declared receiver.
        Intent button = new Intent(Intent.ACTION_MEDIA_BUTTON, null, this, SonoraMediaService.class);
        session.setMediaButtonReceiver(PendingIntent.getService(this, 0, button,
                PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT));
        session.setCallback(new MediaSessionCompat.Callback() {
            @Override public void onPlay() { nativeMediaAction(MEDIA_PLAY); }
            @Override public void onPause() { nativeMediaAction(MEDIA_PAUSE); }
            @Override public void onSkipToNext() { nativeMediaAction(MEDIA_NEXT); }
            @Override public void onSkipToPrevious() { nativeMediaAction(MEDIA_PREVIOUS); }
            @Override public void onSeekTo(long to) { nativeMediaSeek(to); }
            @Override public void onStop() { nativeMediaAction(MEDIA_PAUSE); }
        });
        session.setActive(true);
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        String action = intent == null ? "" : intent.getAction();

        if (ACTION_STOP.equals(action)) {
            stopForeground(true);
            stopSelf();
            return START_NOT_STICKY;
        }
        // A media button delivered through the declared receiver.
        if (Intent.ACTION_MEDIA_BUTTON.equals(action)) {
            KeyEvent key = intent.getParcelableExtra(Intent.EXTRA_KEY_EVENT);
            if (key != null && key.getAction() == KeyEvent.ACTION_DOWN) {
                nativeMediaKey(key.getKeyCode());
            }
            return START_STICKY;
        }
        if (ACTION_PLAY.equals(action)) { nativeMediaAction(MEDIA_PLAY); return START_STICKY; }
        if (ACTION_PAUSE.equals(action)) { nativeMediaAction(MEDIA_PAUSE); return START_STICKY; }
        if (ACTION_NEXT.equals(action)) { nativeMediaAction(MEDIA_NEXT); return START_STICKY; }
        if (ACTION_PREVIOUS.equals(action)) { nativeMediaAction(MEDIA_PREVIOUS); return START_STICKY; }
        if (intent == null) {
            return START_STICKY;
        }

        title = or(intent.getStringExtra("title"));
        artist = or(intent.getStringExtra("artist"));
        album = or(intent.getStringExtra("album"));
        cover = or(intent.getStringExtra("cover"));
        playing = intent.getBooleanExtra("playing", false);
        position = intent.getLongExtra("position", 0);
        duration = intent.getLongExtra("duration", 0);

        session.setPlaybackState(playbackState());
        session.setMetadata(metadata());
        showNotification();
        loadCover();
        return START_STICKY;
    }

    private void showNotification() {
        Notification notification = builder().build();
        if (Build.VERSION.SDK_INT >= 29) {
            startForeground(NOTIFICATION_ID, notification,
                    ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK);
        } else {
            startForeground(NOTIFICATION_ID, notification);
        }
    }

    private NotificationCompat.Builder builder() {
        Intent open = new Intent(this, SonoraActivity.class);
        open.setFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP);
        PendingIntent content = PendingIntent.getActivity(this, 0, open,
                PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);

        NotificationCompat.Builder builder = new NotificationCompat.Builder(this, CHANNEL)
                .setSmallIcon(android.R.drawable.ic_media_play)
                .setContentTitle(title)
                .setContentText(artist)
                .setSubText(album)
                .setContentIntent(content)
                .setOnlyAlertOnce(true)
                .setOngoing(playing)
                .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
                .addAction(action(ACTION_PREVIOUS, "Previous", android.R.drawable.ic_media_previous))
                .addAction(action(playing ? ACTION_PAUSE : ACTION_PLAY,
                        playing ? "Pause" : "Play",
                        playing ? android.R.drawable.ic_media_pause : android.R.drawable.ic_media_play))
                .addAction(action(ACTION_NEXT, "Next", android.R.drawable.ic_media_next))
                .addAction(action(ACTION_STOP, "Stop", android.R.drawable.ic_menu_close_clear_cancel))
                .setStyle(new androidx.media.app.NotificationCompat.MediaStyle()
                        .setMediaSession(session.getSessionToken())
                        .setShowActionsInCompactView(0, 1, 2));

        if (coverBitmap != null) {
            builder.setLargeIcon(coverBitmap);
        }
        return builder;
    }

    private NotificationCompat.Action action(String name, String label, int icon) {
        Intent intent = new Intent(this, SonoraMediaService.class).setAction(name);
        PendingIntent pending = PendingIntent.getService(this, name.hashCode(), intent,
                PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        return new NotificationCompat.Action.Builder(icon, label, pending).build();
    }

    private PlaybackStateCompat playbackState() {
        long actions = PlaybackStateCompat.ACTION_PLAY_PAUSE
                | PlaybackStateCompat.ACTION_SKIP_TO_NEXT
                | PlaybackStateCompat.ACTION_SKIP_TO_PREVIOUS
                | PlaybackStateCompat.ACTION_SEEK_TO;
        int state = playing
                ? PlaybackStateCompat.STATE_PLAYING
                : PlaybackStateCompat.STATE_PAUSED;
        return new PlaybackStateCompat.Builder()
                .setActions(actions)
                .setState(state, position, playing ? 1f : 0f)
                .build();
    }

    private MediaMetadataCompat metadata() {
        return new MediaMetadataCompat.Builder()
                .putString(MediaMetadataCompat.METADATA_KEY_TITLE, title)
                .putString(MediaMetadataCompat.METADATA_KEY_ARTIST, artist)
                .putString(MediaMetadataCompat.METADATA_KEY_ALBUM, album)
                .putLong(MediaMetadataCompat.METADATA_KEY_DURATION, duration)
                .build();
    }

    /** Covers are fetched off the UI thread; a failure just leaves the icon out. */
    private void loadCover() {
        if (cover == null || cover.isEmpty() || !cover.startsWith("http")) {
            return;
        }
        final String url = cover;
        new Thread(() -> {
            try {
                Bitmap bitmap = BitmapFactory.decodeStream(
                        new URL(url).openConnection().getInputStream());
                if (bitmap != null && current == this) {
                    coverBitmap = bitmap;
                    showNotification();
                }
            } catch (Exception ignored) {
                // No cover is not worth a crash or a retry loop.
            }
        }).start();
    }

    private static String or(String value) {
        return value == null ? "" : value;
    }

    @Override
    public void onDestroy() {
        if (session != null) {
            session.setActive(false);
            session.release();
        }
        if (current == this) {
            current = null;
        }
        super.onDestroy();
    }

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }

    // ── JNI entry points, called from crates/state/src/remote/android.rs ────

    private static native void nativeMediaAction(int action);

    private static native void nativeMediaSeek(long positionMs);

    /**
     * A media key the activity saw. Some headsets deliver play/pause only to the
     * focused activity rather than through the session.
     */
    static native void nativeMediaKey(int keyCode);
}
