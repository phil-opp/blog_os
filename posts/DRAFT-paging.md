---
layout: post
title: 'A Paging Module'
---

## Recursive Mapping
The trick is to map the `P4` table _recursively_: The last entry doesn't point to a `P3` table, instead it points to the `P4` table itself. Through this entry, we can access and modify page tables of all levels. It may seem a bit strange at first, but is a very clean and simple solution once you wrapped your head around it.

To access for example the `P4` table itself, we use the address that chooses the 511th `P4` entry, the 511th `P3` entry, the 511th `P2` entry and the 511th `P1` entry. Thus we choose the same `P4` frame over and over again and finally end up on it, too. Through the offset (12 bits) we choose the desired entry.

To access a `P3` table, we do the same but choose the real `P4` index instead of the fourth loop. So if we like to access the 42th `P3` table, we use the address that chooses the 511th entry in the `P4`, `P3`, and `P2` table, but the 42th `P1` entry.

When accessing a `P2` table, we only loop two times and then choose entries that correspond to the `P4` and `P3` table of the desired `P2` table. And accessing a `P1` table just loops once and then uses the corresponding `P4`, `P3`, and `P2` entries.

The math checks out, too. If all page tables are used, there is 1 `P4` table, 511 `P3` tables (the last entry is used for the recursive mapping), `511*512` `P2` tables, and `511*512*512` `P1` tables. So there are `134217728` page tables altogether. Each page table occupies 4KiB, so we need `134217728 * 4KiB = 512GiB` to store them. That's exactly the amount of memory that can be accessed through one `P4` entry since `4KiB per page * 512 P1 entries * 512 P2 entries * 512 P3 entries = 512GiB`.

## A Safe Module

## Switching Page Tables

## Mapping Pages

## Unmapping Pages
