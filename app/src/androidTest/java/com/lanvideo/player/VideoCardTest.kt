package com.lanvideo.player

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import com.lanvideo.player.ui.components.VideoCard
import com.lanvideo.player.ui.home.VideoItem
import com.lanvideo.player.ui.theme.KawaiiTheme
import org.junit.Rule
import org.junit.Test

class VideoCardTest {

    @get:Rule
    val composeTestRule = createComposeRule()

    private fun sampleVideo(
        title: String = "测试视频",
        category: String = "general",
        icon: String = "🎬"
    ) = VideoItem(
        id = "1",
        title = title,
        thumbnailUrl = "",
        sourceType = "local_video",
        category = category,
        views = 42,
        timestamp = "03:25",
        icon = icon
    )

    @Test
    fun videoCard_displaysTitle() {
        val video = sampleVideo(title = "我的测试视频标题")
        composeTestRule.setContent {
            KawaiiTheme {
                VideoCard(video = video, onClick = {})
            }
        }
        composeTestRule.onNodeWithText("我的测试视频标题").assertIsDisplayed()
    }

    @Test
    fun videoCard_displaysCategoryTag() {
        val video = sampleVideo(category = "music")
        composeTestRule.setContent {
            KawaiiTheme {
                VideoCard(video = video, onClick = {})
            }
        }
        composeTestRule.onNodeWithText("music").assertIsDisplayed()
    }

    @Test
    fun videoCard_displaysTimestampAndViews() {
        val video = sampleVideo()
        composeTestRule.setContent {
            KawaiiTheme {
                VideoCard(video = video, onClick = {})
            }
        }
        composeTestRule.onNodeWithText("03:25 · 42 观看").assertIsDisplayed()
    }

    @Test
    fun videoCard_displaysIconWhenNoThumbnail() {
        val video = sampleVideo(icon = "🎵")
        composeTestRule.setContent {
            KawaiiTheme {
                VideoCard(video = video, onClick = {})
            }
        }
        // When thumbnailUrl is blank, the icon emoji is shown
        composeTestRule.onNodeWithText("🎵").assertIsDisplayed()
    }

    @Test
    fun videoCard_generalCategoryShowsTag() {
        val video = sampleVideo(category = "general")
        composeTestRule.setContent {
            KawaiiTheme {
                VideoCard(video = video, onClick = {})
            }
        }
        composeTestRule.onNodeWithText("general").assertIsDisplayed()
    }
}
