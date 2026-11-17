/*
 * surface.h
 * Copyright 2011 John Lindgren
 *
 * This file is part of Audacious.
 *
 * Audacious is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, version 2 or version 3 of the License.
 *
 * Audacious is distributed in the hope that it will be useful, but WITHOUT ANY
 * WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR
 * A PARTICULAR PURPOSE. See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * Audacious. If not, see <http://www.gnu.org/licenses/>.
 *
 * The Audacious team does not consider modular code linking to Audacious or
 * using our public API to be a derived work.
 */

#ifndef SKINS_SURFACE_H
#define SKINS_SURFACE_H

#include <stdint.h>
#include <cairo.h>

cairo_surface_t * surface_new (int w, int h);
cairo_surface_t * surface_new_from_file (const char * name);
uint32_t surface_get_pixel (cairo_surface_t * s, int x, int y);
void surface_copy_rect (cairo_surface_t * a, int ax, int ay, int w, int h,
 cairo_surface_t * b, int bx, int by);

/* A copy of an image surface that is kept in the X server (as a pixmap), so
 * that drawing from it is a copy inside the server (and on the GPU, where the
 * server draws with it) rather than an upload of the pixels on each redraw.
 *
 * The copy is made when it is first drawn from, for the display, the window
 * scale and the scale factor that the cairo context draws at; it is made anew
 * when one of these has changed.  It is scaled in advance, so that it is drawn
 * pixel for pixel.
 *
 * Where the window scale and a smoothing filter blur the image, the blurred
 * edge reaches past the image and is partly transparent.  The copy then has
 * an alpha channel and a transparent border of border() pixels around it, so
 * that drawing from it gives what drawing from the image gives. */
class ServerSurface
{
public:
    /* Returns the copy of <image>, <scale> times its size, or null if what
     * <cr> draws to is not in an X server (the caller draws from <image>). */
    cairo_surface_t * get (cairo_t * cr, cairo_surface_t * image, int scale,
     cairo_filter_t filter);

    /* the width of the border around the copy that get() returned */
    int border () const { return m_border; }

    void clear ();

    ServerSurface () {}
    ~ServerSurface () { clear (); }

    ServerSurface (ServerSurface && b) :
        m_surface (b.m_surface), m_device (b.m_device),
        m_device_scale (b.m_device_scale), m_scale (b.m_scale),
        m_border (b.m_border)
    {
        b.m_surface = nullptr;
        b.m_device = nullptr;
    }

    ServerSurface & operator= (ServerSurface && b)
    {
        if (this != & b)
        {
            clear ();
            m_surface = b.m_surface;
            m_device = b.m_device;
            m_device_scale = b.m_device_scale;
            m_scale = b.m_scale;
            m_border = b.m_border;
            b.m_surface = nullptr;
            b.m_device = nullptr;
        }
        return * this;
    }

private:
    cairo_surface_t * m_surface = nullptr;
    cairo_device_t * m_device = nullptr;
    double m_device_scale = 0;
    int m_scale = 0;
    int m_border = 0;
};

/* Draws the part <xsrc, ysrc, width, height> of <image> at <xdest, ydest>,
 * from its copy in the X server if there can be one. */
void surface_draw_rect (cairo_t * cr, cairo_surface_t * image,
 ServerSurface & copy, int xsrc, int ysrc, int xdest, int ydest, int width,
 int height);

#endif
