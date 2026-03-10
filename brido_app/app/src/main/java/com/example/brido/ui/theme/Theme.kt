package com.example.brido.ui.theme

import android.app.Activity
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalView
import androidx.core.view.WindowCompat

private val BridoColorScheme = darkColorScheme(
    primary = BridoAccent,
    secondary = BridoSurfaceVariant,
    tertiary = BridoInfoBlue,
    background = BridoDark,
    surface = BridoSurface,
    onPrimary = Color.White,
    onSecondary = BridoTextPrimary,
    onTertiary = Color.White,
    onBackground = BridoTextPrimary,
    onSurface = BridoTextPrimary,
)

@Composable
fun BridoTheme(
    content: @Composable () -> Unit,
) {
    val view = LocalView.current
    if (!view.isInEditMode) {
        SideEffect {
            val window = (view.context as Activity).window
            window.statusBarColor = BridoDark.toArgb()
            window.navigationBarColor = BridoDark.toArgb()
            WindowCompat.getInsetsController(window, view).isAppearanceLightStatusBars = false
        }
    }

    MaterialTheme(
        colorScheme = BridoColorScheme,
        typography = Typography,
        content = content,
    )
}