package com.lanvideo.player.data.util

import android.content.ContentResolver
import android.net.Uri
import android.provider.OpenableColumns

fun queryDisplayName(cr: ContentResolver, uri: Uri): String? {
    if (uri.scheme == ContentResolver.SCHEME_CONTENT) {
        cr.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
            if (c.moveToFirst()) {
                val i = c.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                if (i >= 0) return c.getString(i)
            }
        }
    }
    return uri.lastPathSegment
}

fun queryFileSize(cr: ContentResolver, uri: Uri): Long {
    if (uri.scheme == ContentResolver.SCHEME_CONTENT) {
        cr.query(uri, arrayOf(OpenableColumns.SIZE), null, null, null)?.use { c ->
            if (c.moveToFirst()) {
                val i = c.getColumnIndex(OpenableColumns.SIZE)
                if (i >= 0 && !c.isNull(i)) return c.getLong(i).coerceAtLeast(0L)
            }
        }
    }
    return -1L
}
