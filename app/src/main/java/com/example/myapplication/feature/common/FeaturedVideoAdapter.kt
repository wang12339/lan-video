package com.example.myapplication.feature.common

import android.view.LayoutInflater
import android.view.ViewGroup
import androidx.core.view.isVisible
import androidx.recyclerview.widget.DiffUtil
import androidx.recyclerview.widget.ListAdapter
import androidx.recyclerview.widget.RecyclerView
import coil.load
import com.example.myapplication.R
import com.example.myapplication.data.model.VideoItem
import com.example.myapplication.data.network.StreamUrlResolver
import com.example.myapplication.databinding.ItemVideoFeaturedBinding

class FeaturedVideoAdapter(
    private val onClick: (VideoItem) -> Unit,
    private val onLongClick: ((VideoItem) -> Unit)? = null
) : ListAdapter<VideoItem, FeaturedVideoAdapter.FeaturedViewHolder>(DiffCallback) {

    private val streamUrlResolver = StreamUrlResolver

    var isSelectMode: Boolean = false
    val selectedIds: MutableSet<Long> = mutableSetOf()

    fun toggleSelection(id: Long): Boolean {
        return if (selectedIds.remove(id)) {
            notifyDataSetChanged()
            false
        } else {
            selectedIds.add(id)
            notifyDataSetChanged()
            true
        }
    }

    fun clearSelection() {
        selectedIds.clear()
        isSelectMode = false
        notifyDataSetChanged()
    }

    fun enterSelectMode(firstId: Long) {
        isSelectMode = true
        if (firstId >= 0) selectedIds.add(firstId)
        notifyDataSetChanged()
    }

    fun selectAll() {
        currentList.forEach { selectedIds.add(it.id) }
        notifyDataSetChanged()
    }

    fun isAllSelected(): Boolean = currentList.isNotEmpty() && selectedIds.size == currentList.size

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): FeaturedViewHolder {
        val binding = ItemVideoFeaturedBinding.inflate(LayoutInflater.from(parent.context), parent, false)
        return FeaturedViewHolder(binding, onClick, onLongClick)
    }

    override fun onBindViewHolder(holder: FeaturedViewHolder, position: Int) {
        holder.bind(getItem(position), isSelectMode, selectedIds)
    }

    inner class FeaturedViewHolder(
        private val binding: ItemVideoFeaturedBinding,
        private val onClick: (VideoItem) -> Unit,
        private val onLongClick: ((VideoItem) -> Unit)?
    ) : RecyclerView.ViewHolder(binding.root) {
        fun bind(item: VideoItem, selectMode: Boolean, selectedSet: Set<Long>) {
            binding.textTitle.text = item.title
            val imgUrl = item.coverUrl?.takeIf { it.isNotBlank() } ?: item.streamUrl
            val absUrl = streamUrlResolver.toAbsoluteStreamUrl(imgUrl)
            binding.imageCover.load(absUrl) {
                placeholder(R.drawable.ic_gallery_black_24dp)
                error(R.drawable.ic_slideshow_black_24dp)
                size(640, 360)
                crossfade(true)
            }
            binding.selectionOverlay.isVisible = selectMode && item.id in selectedSet

            // Duration badge
            if (item.duration > 0) {
                binding.badgeDuration.isVisible = true
                binding.badgeDuration.text = formatDuration(item.duration)
            } else {
                binding.badgeDuration.isVisible = false
            }

            // Watch progress
            if (item.duration > 0) {
                val progress = ((item.watchPosition.toFloat() / item.duration) * 100).toInt().coerceIn(0, 100)
                if (progress in 1..94) {
                    binding.progressWatched.isVisible = true
                    val lp = binding.progressWatched.layoutParams
                    lp.width = (binding.imageCover.width.toFloat() * progress / 100f).toInt().coerceAtLeast(0)
                    binding.progressWatched.layoutParams = lp
                } else {
                    binding.progressWatched.isVisible = false
                }
            } else {
                binding.progressWatched.isVisible = false
            }

            if (selectMode) {
                binding.root.setOnClickListener { onClick(item) }
                binding.root.setOnLongClickListener(null)
            } else {
                binding.root.setOnClickListener { onClick(item) }
                if (onLongClick != null) {
                    binding.root.setOnLongClickListener { onLongClick(item); true }
                } else {
                    binding.root.setOnLongClickListener(null)
                }
            }
        }
    }

    private fun formatDuration(ms: Long): String {
        val totalSec = ms / 1000
        val min = totalSec / 60
        val sec = totalSec % 60
        return "%d:%02d".format(min, sec)
    }

    private object DiffCallback : DiffUtil.ItemCallback<VideoItem>() {
        override fun areItemsTheSame(oldItem: VideoItem, newItem: VideoItem): Boolean = oldItem.id == newItem.id
        override fun areContentsTheSame(oldItem: VideoItem, newItem: VideoItem): Boolean = oldItem == newItem
    }
}
