package com.lanvideo.player.util

import org.junit.Assert.assertEquals
import org.junit.Test

class VideoFormattersTest {

    // ── formatDuration ──

    @Test
    fun `formatDuration returns 刚刚 for zero millis`() {
        assertEquals("刚刚", VideoFormatters.formatDuration(0L))
    }

    @Test
    fun `formatDuration returns 刚刚 for sub-minute duration`() {
        assertEquals("刚刚", VideoFormatters.formatDuration(1000L))   // 1s
        assertEquals("刚刚", VideoFormatters.formatDuration(30_000L)) // 30s
        assertEquals("刚刚", VideoFormatters.formatDuration(59_000L)) // 59s
    }

    @Test
    fun `formatDuration returns minutes for minute-range duration`() {
        assertEquals("1分钟前", VideoFormatters.formatDuration(60_000L))      // exactly 1 min
        assertEquals("2分钟前", VideoFormatters.formatDuration(120_000L))     // 2 min
        assertEquals("59分钟前", VideoFormatters.formatDuration(3_540_000L))  // 59 min
    }

    @Test
    fun `formatDuration returns hours for hour-range duration`() {
        assertEquals("1小时前", VideoFormatters.formatDuration(3_600_000L))    // exactly 1 hour
        assertEquals("2小时前", VideoFormatters.formatDuration(7_200_000L))    // 2 hours
        assertEquals("24小时前", VideoFormatters.formatDuration(86_400_000L))  // 24 hours
    }

    @Test
    fun `formatDuration truncates to whole units`() {
        // 1 hour + 30 minutes -> displays as hours (integer division: 5400s / 3600 = 1)
        assertEquals("1小时前", VideoFormatters.formatDuration(5_400_000L))
    }

    // ── getCategoryIcon ──

    @Test
    fun `getCategoryIcon returns correct icon for Chinese categories`() {
        assertEquals("🐰", VideoFormatters.getCategoryIcon("动画"))   // rabbit
        assertEquals("🐶", VideoFormatters.getCategoryIcon("萌宠"))   // dog
        assertEquals("🐱", VideoFormatters.getCategoryIcon("搞笑"))   // cat
        assertEquals("🦊", VideoFormatters.getCategoryIcon("治愈"))   // fox
        assertEquals("🎵", VideoFormatters.getCategoryIcon("音乐"))   // music
        assertEquals("🎮", VideoFormatters.getCategoryIcon("游戏"))   // game
        assertEquals("🔬", VideoFormatters.getCategoryIcon("科技"))   // microscope
        assertEquals("🎨", VideoFormatters.getCategoryIcon("设计"))   // art
        assertEquals("📚", VideoFormatters.getCategoryIcon("教程"))   // books
        assertEquals("🎬", VideoFormatters.getCategoryIcon("娱乐"))   // clapper
        assertEquals("⚽", VideoFormatters.getCategoryIcon("运动"))
        assertEquals("📷", VideoFormatters.getCategoryIcon("记录"))   // camera
    }

    @Test
    fun `getCategoryIcon returns correct icon for English categories`() {
        assertEquals("🐰", VideoFormatters.getCategoryIcon("animation"))
        assertEquals("🐶", VideoFormatters.getCategoryIcon("pet"))
        assertEquals("🐱", VideoFormatters.getCategoryIcon("funny"))
        assertEquals("🦊", VideoFormatters.getCategoryIcon("healing"))
        assertEquals("🎵", VideoFormatters.getCategoryIcon("music"))
        assertEquals("🎮", VideoFormatters.getCategoryIcon("game"))
    }

    @Test
    fun `getCategoryIcon is case insensitive for English`() {
        assertEquals("🐰", VideoFormatters.getCategoryIcon("Animation"))
        assertEquals("🐶", VideoFormatters.getCategoryIcon("PET"))
        assertEquals("🐱", VideoFormatters.getCategoryIcon("Funny"))
    }

    @Test
    fun `getCategoryIcon returns default for unknown category`() {
        assertEquals("🎬", VideoFormatters.getCategoryIcon("unknown"))
        assertEquals("🎬", VideoFormatters.getCategoryIcon(""))
        assertEquals("🎬", VideoFormatters.getCategoryIcon("science"))
    }
}
