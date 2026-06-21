package com.lanvideo.player.ui.components

import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import com.lanvideo.player.ui.navigation.Screen

@Composable
fun BottomNavBar(
    currentRoute: String?,
    onNavigate: (Screen) -> Unit
) {
    val items = listOf(
        Screen.Home to "首页",
        Screen.Search to "搜索",
        Screen.User to "我的"
    )

    NavigationBar {
        items.forEach { (screen, label) ->
            NavigationBarItem(
                selected = currentRoute == screen.route,
                onClick = { onNavigate(screen) },
                icon = {},
                label = { Text(label) }
            )
        }
    }
}
