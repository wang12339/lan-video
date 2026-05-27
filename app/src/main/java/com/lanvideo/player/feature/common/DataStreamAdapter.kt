package com.lanvideo.player.feature.common

import android.view.LayoutInflater
import android.view.ViewGroup
import androidx.core.view.isVisible
import androidx.recyclerview.widget.DiffUtil
import androidx.recyclerview.widget.ListAdapter
import androidx.recyclerview.widget.RecyclerView
import coil.load
import com.lanvideo.player.R
import com.lanvideo.player.data.model.VideoItem
import com.lanvideo.player.data.network.StreamUrlResolver
import com.lanvideo.player.databinding.ItemDataSlabBinding

class DataStreamAdapter(
    private val onClick: (VideoItem) -> Unit,
    private val onLongClick: ((VideoItem) -> Unit)? = null
) : ListAdapter<VideoItem, DataStreamAdapter.SlabViewHolder>(DiffCallback) {

    private val streamUrlResolver = StreamUrlResolver
    private val internalSelectedIds = mutableSetOf<Long>()

    val selectedIds: Set<Long> get() = internalSelectedIds

    var isSelectMode: Boolean = false

    fun toggleSelection(id: Long): Boolean {
        return if (internalSelectedIds.remove(id)) {
            notifyDataSetChanged()
            false
        } else {
            internalSelectedIds.add(id)
            notifyDataSetChanged()
            true
        }
    }

    fun clearSelection() {
        internalSelectedIds.clear()
        isSelectMode = false
        notifyDataSetChanged()
    }

    fun enterSelectMode(firstId: Long) {
        isSelectMode = true
        if (firstId >= 0) internalSelectedIds.add(firstId)
        notifyDataSetChanged()
    }

    fun selectAll() {
        currentList.forEach { internalSelectedIds.add(it.id) }
        notifyDataSetChanged()
    }

    fun isAllSelected(): Boolean = currentList.isNotEmpty() && internalSelectedIds.size == currentList.size

    val selectedCount: Int get() = internalSelectedIds.size

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): SlabViewHolder {
        val binding = ItemDataSlabBinding.inflate(LayoutInflater.from(parent.context), parent, false)
        return SlabViewHolder(binding, onClick, onLongClick)
    }

    override fun onBindViewHolder(holder: SlabViewHolder, position: Int) {
        holder.bind(getItem(position), isSelectMode, internalSelectedIds)
    }

    inner class SlabViewHolder(
        private val binding: ItemDataSlabBinding,
        private val onClick: (VideoItem) -> Unit,
        private val onLongClick: ((VideoItem) -> Unit)?
    ) : RecyclerView.ViewHolder(binding.root) {
        fun bind(item: VideoItem, selectMode: Boolean, selectedSet: Set<Long>) {
            binding.thumbnailContainer.post {
                val w = binding.thumbnailContainer.width
                if (w > 0) {
                    val lp = binding.thumbnailContainer.layoutParams
                    lp.height = (w * 9f / 16f).toInt()
                    binding.thumbnailContainer.layoutParams = lp

                    // Watch progress bar — calculated after layout for correct width
                    if (item.duration > 0) {
                        val progress = ((item.watchPosition?.toFloat() ?: 0f) / item.duration * 100).toInt().coerceIn(0, 100)
                        if (progress in 1..94) {
                            binding.progressWatched.isVisible = true
                            val plp = binding.progressWatched.layoutParams
                            plp.width = (w.toFloat() * progress / 100f).toInt().coerceAtLeast(0)
                            binding.progressWatched.layoutParams = plp
                        } else {
                            binding.progressWatched.isVisible = false
                        }
                    } else {
                        binding.progressWatched.isVisible = false
                    }
                }
            }

            val imgUrl = item.coverUrl?.takeIf { it.isNotBlank() } ?: item.streamUrl
            binding.imageCover.load(streamUrlResolver.toAbsoluteStreamUrl(imgUrl)) {
                placeholder(R.drawable.ic_gallery_black_24dp)
                error(R.drawable.ic_slideshow_black_24dp)
                size(640, 360)
                crossfade(true)
            }

            binding.textTitle.text = "${item.title}"
            binding.textMetadata.text = "分类: ${item.category}  |  ${formatDuration(item.duration)}"

            // Selection
            binding.selectionOverlay.isVisible = selectMode && item.id in selectedSet

            // Duration badge
            if (item.duration > 0) {
                binding.badgeDuration.isVisible = true
                binding.badgeDuration.text = formatDuration(item.duration)
            } else {
                binding.badgeDuration.isVisible = false
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
        if (ms <= 0) return "--:--"
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
