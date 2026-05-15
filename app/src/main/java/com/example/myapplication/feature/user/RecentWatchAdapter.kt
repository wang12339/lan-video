package com.example.myapplication.feature.user

import android.view.LayoutInflater
import android.view.ViewGroup
import androidx.core.view.isVisible
import androidx.recyclerview.widget.DiffUtil
import androidx.recyclerview.widget.ListAdapter
import androidx.recyclerview.widget.RecyclerView
import coil.load
import com.example.myapplication.R
import com.example.myapplication.data.model.RecentWatchItem
import com.example.myapplication.data.network.StreamUrlResolver
import com.example.myapplication.databinding.ItemRecentWatchBinding

class RecentWatchAdapter(
    private val onClick: (RecentWatchItem) -> Unit
) : ListAdapter<RecentWatchItem, RecentWatchAdapter.ViewHolder>(DiffCallback) {

    private val streamUrlResolver = StreamUrlResolver

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): ViewHolder {
        val binding = ItemRecentWatchBinding.inflate(
            LayoutInflater.from(parent.context), parent, false
        )
        return ViewHolder(binding, onClick, streamUrlResolver)
    }

    override fun onBindViewHolder(holder: ViewHolder, position: Int) {
        holder.bind(getItem(position))
    }

    inner class ViewHolder(
        private val binding: ItemRecentWatchBinding,
        private val onClick: (RecentWatchItem) -> Unit,
        private val streamUrlResolver: StreamUrlResolver
    ) : RecyclerView.ViewHolder(binding.root) {

        fun bind(item: RecentWatchItem) {
            binding.itemTitle.text = item.title

            val imgUrl = item.coverUrl?.takeIf { it.isNotBlank() } ?: item.streamUrl
            val absUrl = streamUrlResolver.toAbsoluteStreamUrl(imgUrl)
            binding.itemCover.load(absUrl) {
                placeholder(R.drawable.ic_gallery_black_24dp)
                error(R.drawable.ic_slideshow_black_24dp)
            }

            val progress = if (item.durationMs > 0) {
                (item.positionMs.toFloat() / item.durationMs * 100).toInt().coerceIn(0, 100)
            } else 0
            binding.itemProgress.max = 100
            binding.itemProgress.progress = progress

            binding.itemTime.text = formatDuration(item.durationMs)
            binding.itemTime.isVisible = item.durationMs > 0

            binding.root.setOnClickListener { onClick(item) }
        }

        private fun formatDuration(ms: Long): String {
            val totalSec = ms / 1000
            if (totalSec <= 0) return ""
            val min = totalSec / 60
            val sec = totalSec % 60
            return if (min > 0) "${min}分${sec}秒" else "${sec}秒"
        }
    }

    private object DiffCallback : DiffUtil.ItemCallback<RecentWatchItem>() {
        override fun areItemsTheSame(oldItem: RecentWatchItem, newItem: RecentWatchItem): Boolean =
            oldItem.videoId == newItem.videoId

        override fun areContentsTheSame(oldItem: RecentWatchItem, newItem: RecentWatchItem): Boolean =
            oldItem == newItem
    }
}
