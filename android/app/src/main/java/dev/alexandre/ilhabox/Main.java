package dev.alexandre.ilhabox;

import android.annotation.SuppressLint;
import android.app.Activity;
import android.content.Intent;
import android.content.SharedPreferences;
import android.graphics.Color;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.os.VibrationEffect;
import android.os.Vibrator;
import android.view.View;
import android.view.WindowInsets;
import android.view.WindowInsetsController;
import android.view.WindowManager;
import android.webkit.JavascriptInterface;
import android.webkit.WebResourceError;
import android.webkit.WebResourceRequest;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;

/**
 * Full-screen panel. The page and all its data come from the IlhaBox on the PC, through the USB
 * tunnel (adb reverse) to 127.0.0.1:8777; this app only shows it, so it never needs an update for
 * a new screen. Nothing else is loaded: any other address is refused.
 */
public class Main extends Activity {
    static final String ORIGIN = "http://127.0.0.1:8777";
    private static Main current;

    private WebView web;
    private String loadedKey = null;
    private final Handler ui = new Handler(Looper.getMainLooper());
    private final Runnable retry = this::load;

    static SharedPreferences prefs(Activity a) {
        return a.getSharedPreferences("ilhabox", MODE_PRIVATE);
    }

    /** Called by {@link Chave} when the PC hands over a new key while the panel is open. */
    static void keyChanged() {
        Main m = current;
        if (m != null) m.ui.post(m::reloadIfKeyChanged);
    }

    @SuppressLint({"SetJavaScriptEnabled", "AddJavascriptInterface"})
    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        current = this;
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);

        web = new WebView(this);
        web.setBackgroundColor(Color.BLACK);
        WebSettings s = web.getSettings();
        s.setJavaScriptEnabled(true);
        s.setDomStorageEnabled(true);
        s.setMediaPlaybackRequiresUserGesture(false);
        s.setSupportZoom(false);
        s.setAllowFileAccess(false);
        s.setAllowContentAccess(false);
        web.addJavascriptInterface(new Bridge(), "IlhaBoxApp");
        web.setWebViewClient(new WebViewClient() {
            @Override
            public boolean shouldOverrideUrlLoading(WebView view, WebResourceRequest req) {
                return !req.getUrl().toString().startsWith(ORIGIN);
            }

            @Override
            public void onReceivedError(WebView view, WebResourceRequest req, WebResourceError err) {
                // The PC is not answering yet (cable just plugged, agent starting): wait and try again.
                if (req.isForMainFrame()) {
                    view.loadDataWithBaseURL(null, WAITING, "text/html", "utf-8", null);
                    ui.removeCallbacks(retry);
                    ui.postDelayed(retry, 2500);
                }
            }
        });
        setContentView(web);
        immersive();
        load();
    }

    private String key() {
        return prefs(this).getString("k", "");
    }

    private void load() {
        loadedKey = key();
        web.loadUrl(ORIGIN + "/#k=" + Uri.encode(loadedKey));
    }

    private void reloadIfKeyChanged() {
        if (!key().equals(loadedKey)) load();
    }

    @Override
    protected void onResume() {
        super.onResume();
        immersive();
        reloadIfKeyChanged();
    }

    @Override
    public void onWindowFocusChanged(boolean focus) {
        super.onWindowFocusChanged(focus);
        if (focus) immersive();
    }

    @SuppressWarnings("deprecation")
    private void immersive() {
        if (Build.VERSION.SDK_INT >= 30) {
            WindowInsetsController c = getWindow().getInsetsController();
            if (c != null) {
                c.hide(WindowInsets.Type.statusBars() | WindowInsets.Type.navigationBars());
                c.setSystemBarsBehavior(WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);
            }
        } else {
            getWindow().getDecorView().setSystemUiVisibility(View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
                    | View.SYSTEM_UI_FLAG_FULLSCREEN | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                    | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
                    | View.SYSTEM_UI_FLAG_LAYOUT_STABLE);
        }
    }

    /** Back does nothing: the panel only leaves through "Celular normal" or the Home button. */
    @SuppressWarnings("deprecation")
    @Override
    public void onBackPressed() {
    }

    @Override
    protected void onDestroy() {
        ui.removeCallbacksAndMessages(null);
        if (current == this) current = null;
        web.destroy();
        super.onDestroy();
    }

    private void goHome() {
        Intent home = new Intent(Intent.ACTION_MAIN);
        home.addCategory(Intent.CATEGORY_HOME);
        home.setFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        startActivity(home);
    }

    /** What the page can ask of the phone itself. */
    final class Bridge {
        @JavascriptInterface
        public void home() {
            ui.post(Main.this::goHome);
        }

        @JavascriptInterface
        @SuppressWarnings("deprecation")
        public void vibrate(int ms) {
            Vibrator v = (Vibrator) getSystemService(VIBRATOR_SERVICE);
            if (v == null || !v.hasVibrator()) return;
            int d = Math.max(10, Math.min(ms, 800));
            if (Build.VERSION.SDK_INT >= 26) v.vibrate(VibrationEffect.createOneShot(d, VibrationEffect.DEFAULT_AMPLITUDE));
            else v.vibrate(d);
        }

        @JavascriptInterface
        @SuppressWarnings("deprecation")
        public String version() {
            try {
                return String.valueOf(getPackageManager().getPackageInfo(getPackageName(), 0).versionCode);
            } catch (Exception e) {
                return "?";
            }
        }
    }

    private static final String WAITING = "<html><body style='margin:0;height:100vh;display:flex;flex-direction:column;"
            + "align-items:center;justify-content:center;background:#07070a;color:#fff;font-family:sans-serif;text-align:center'>"
            + "<div style='font-size:20px;font-weight:700'>Procurando o PC…</div>"
            + "<div style='opacity:.55;margin-top:8px'>Ligue o cabo USB e abra a IlhaBox no computador.</div>"
            + "<button onclick='IlhaBoxApp.home()' style='margin-top:22px;padding:12px 22px;border:0;border-radius:14px;"
            + "background:#222;color:#fff;font-size:15px;font-weight:700'>Usar o celular</button></body></html>";
}
