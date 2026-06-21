package com.lanvideo.player.ui.theme

import android.app.Activity
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalView
import androidx.core.view.WindowCompat

private val KawaiiColorScheme = lightColorScheme(
    primary = SakuraPink,
    onPrimary = TextPrimary,
    primaryContainer = BackgroundPink,
    secondary = SkyBlue,
    onSecondary = TextPrimary,
    secondaryContainer = BackgroundBlue,
    tertiary = CreamYellow,
    onTertiary = TextPrimary,
    tertiaryContainer = BackgroundYellow,
    background = BackgroundPink,
    onBackground = TextPrimary,
    surface = BackgroundPink,
    onSurface = TextPrimary,
    surfaceVariant = Lavender,
    onSurfaceVariant = TextSecondary,
)

@Composable
fun KawaiiTheme(
    content: @Composable () -> Unit
) {
    val colorScheme = KawaiiColorScheme

    val view = LocalView.current
    if (!view.isInEditMode) {
        SideEffect {
            val window = (view.context as Activity).window
            window.statusBarColor = BackgroundPink.toArgb()
            WindowCompat.getInsetsController(window, view).isAppearanceLightStatusBars = true
        }
    }

    MaterialTheme(
        colorScheme = colorScheme,
        typography = Typography,
        content = content
    )
}
