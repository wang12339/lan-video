package com.lanvideo.player

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import com.lanvideo.player.ui.navigation.AppNavigation
import com.lanvideo.player.ui.theme.KawaiiTheme

class MainActivity : ComponentActivity() {

    fun openDrawer() {}

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            KawaiiTheme {
                AppNavigation()
            }
        }
    }
}
