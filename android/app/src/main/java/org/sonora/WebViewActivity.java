package org.sonora;

import android.app.Activity;
import android.content.Intent;
import android.graphics.Bitmap;
import android.graphics.Color;
import android.net.Uri;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.view.ViewGroup;
import android.webkit.CookieManager;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.TextView;

/**
 * A throwaway browser window for cookie sign-ins and proof-of-origin tokens.
 *
 * The data store lives only as long as the activity, so nothing rotates the
 * cookies behind the app's back. Rust polls the static state below; when the
 * proof cookies appear it closes the activity itself.
 */
public class WebViewActivity extends Activity {

    public static final String EXTRA_URL = "org.sonora.URL";
    public static final String EXTRA_DOMAIN = "org.sonora.DOMAIN";
    public static final String EXTRA_TITLE = "org.sonora.TITLE";
    public static final String EXTRA_AGENT = "org.sonora.AGENT";
    public static final String EXTRA_SCRIPT = "org.sonora.SCRIPT";

    /** Polled from Rust. */
    public static volatile String currentHost = "";

    /** The `Cookie` header for the page's own url, polled from Rust. */
    public static volatile String currentCookies = "";

    public static volatile boolean finished = false;

    private static WebViewActivity current;

    private WebView webView;
    private Handler handler;
    private String pageUrl;
    private String domain = "";

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        current = this;
        finished = false;
        currentHost = "";
        currentCookies = "";

        Intent intent = getIntent();
        String url = intent.getStringExtra(EXTRA_URL);
        String title = intent.getStringExtra(EXTRA_TITLE);
        String agent = intent.getStringExtra(EXTRA_AGENT);
        String script = intent.getStringExtra(EXTRA_SCRIPT);
        pageUrl = url == null ? "" : url;
        domain = intent.getStringExtra(EXTRA_DOMAIN);
        if (domain == null) {
            domain = "";
        }

        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setBackgroundColor(Color.BLACK);

        if (title != null && !title.isEmpty()) {
            int pad = (int) (12 * getResources().getDisplayMetrics().density);
            LinearLayout bar = new LinearLayout(this);
            bar.setOrientation(LinearLayout.HORIZONTAL);
            bar.setGravity(android.view.Gravity.CENTER_VERTICAL);
            bar.setPadding(pad, pad, pad, pad);

            TextView label = new TextView(this);
            label.setText(title);
            label.setTextColor(Color.WHITE);
            label.setLayoutParams(new LinearLayout.LayoutParams(
                    0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));

            Button close = new Button(this);
            close.setText("✕");
            close.setOnClickListener(v -> finish());

            bar.addView(label);
            bar.addView(close);
            root.addView(bar);
        }

        webView = new WebView(this);
        WebSettings settings = webView.getSettings();
        settings.setJavaScriptEnabled(true);
        settings.setDomStorageEnabled(true);
        if (agent != null && !agent.isEmpty()) {
            settings.setUserAgentString(agent);
        }
        webView.setWebViewClient(new WebViewClient() {
            @Override
            public void onPageStarted(WebView view, String url, Bitmap favicon) {
                currentHost = host(url);
                run(script, view, url);
            }

            @Override
            public void onPageFinished(WebView view, String url) {
                currentHost = host(url);
                run(script, view, url);
            }
        });
        webView.setLayoutParams(new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f));
        root.addView(webView);
        setContentView(root);

        CookieManager cookies = CookieManager.getInstance();
        cookies.setAcceptCookie(true);
        cookies.setAcceptThirdPartyCookies(webView, true);
        if (url != null) {
            webView.loadUrl(url);
        }

        handler = new Handler(Looper.getMainLooper());
        handler.postDelayed(this::tick, 250);
    }

    private static void run(String script, WebView view, String url) {
        if (script != null && !script.isEmpty() && url != null) {
            view.evaluateJavascript(script, null);
        }
    }

    private static String host(String url) {
        if (url == null) {
            return "";
        }
        Uri uri = Uri.parse(url);
        return uri.getHost() == null ? "" : uri.getHost();
    }

    private void tick() {
        if (isFinishing() || isDestroyed()) {
            return;
        }
        // `getCookie(url)` answers with the cookies that url would be sent, so a
        // single query on the sign-in page misses the ones set on the provider's
        // own domain. Ask every URL that could hold them and merge.
        java.util.LinkedHashSet<String> pairs = new java.util.LinkedHashSet<>();
        for (String url : candidates()) {
            String cookie = CookieManager.getInstance().getCookie(url);
            if (cookie == null) {
                continue;
            }
            for (String pair : cookie.split(";")) {
                String trimmed = pair.trim();
                if (!trimmed.isEmpty()) {
                    pairs.add(trimmed);
                }
            }
        }
        currentCookies = String.join("; ", pairs);
        handler.postDelayed(this::tick, 250);
    }

    private java.util.List<String> candidates() {
        java.util.List<String> urls = new java.util.ArrayList<>();
        if (pageUrl != null && !pageUrl.isEmpty()) {
            urls.add(pageUrl);
        }
        String bare = domain.replaceFirst("^\\.", "");
        if (!bare.isEmpty()) {
            urls.add("https://" + bare + "/");
            urls.add("https://www." + bare + "/");
        }
        return urls;
    }

    private void load(String url) {
        pageUrl = url;
        if (webView != null && url != null) {
            webView.loadUrl(url);
        }
    }

    private void run(String script) {
        if (webView != null && script != null) {
            webView.evaluateJavascript(script, null);
        }
    }

    @Override
    public void onBackPressed() {
        if (webView != null && webView.canGoBack()) {
            webView.goBack();
        } else {
            super.onBackPressed();
        }
    }

    @Override
    protected void onDestroy() {
        finished = true;
        if (handler != null) {
            handler.removeCallbacksAndMessages(null);
        }
        if (current == this) {
            current = null;
        }
        super.onDestroy();
    }

    // ── JNI entry points, called from crates/webview/src/android.rs ─────────

    public static String webViewHost() {
        return currentHost;
    }

    public static String webViewCookies() {
        return currentCookies;
    }

    public static boolean webViewClosed() {
        return finished;
    }

    public static void webViewLoad(String url) {
        if (current != null) {
            current.load(url);
        }
    }

    public static void webViewRun(String script) {
        if (current != null) {
            current.run(script);
        }
    }

    public static void webViewClose() {
        if (current != null) {
            current.finish();
        }
    }
}
