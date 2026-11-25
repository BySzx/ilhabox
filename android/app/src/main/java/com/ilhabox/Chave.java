package com.ilhabox;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;

/**
 * The PC sends the API key here over adb each time the phone is plugged in
 * (am broadcast -n com.ilhabox/.Chave --es k KEY). Protected by the DUMP permission,
 * which only the adb shell holds, so no other app on the phone can plant a key.
 */
public class Chave extends BroadcastReceiver {
    @Override
    public void onReceive(Context context, Intent intent) {
        String k = intent.getStringExtra("k");
        if (k == null || k.length() < 32) return;
        context.getSharedPreferences("ilhabox", Context.MODE_PRIVATE).edit().putString("k", k).apply();
        Main.keyChanged();
    }
}
