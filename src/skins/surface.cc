/*
 * surface.c
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

#include <gdk/gdk.h>

#include "surface.h"

#include <libaudcore/runtime.h>
#include <libaudgui/libaudgui-gtk.h>

#include "skins_cfg.h"

cairo_surface_t * surface_new (int w, int h)
{
    return cairo_image_surface_create (CAIRO_FORMAT_RGB24, w, h);
}

cairo_surface_t * surface_new_from_file (const char * name)
{
    GError * error = nullptr;
    AudguiPixbuf p (gdk_pixbuf_new_from_file (name, & error));

    if (error)
    {
        AUDERR ("Error loading %s: %s.\n", name, error->message);
        g_error_free (error);
    }

    if (! p)
        return nullptr;

    cairo_surface_t * surface = surface_new (p.width (), p.height ());
    cairo_t * cr = cairo_create (surface);

    gdk_cairo_set_source_pixbuf (cr, p.get (), 0, 0);
    cairo_paint (cr);

    cairo_destroy (cr);
    return surface;
}

uint32_t surface_get_pixel (cairo_surface_t * s, int x, int y)
{
    if (x < 0 || x >= cairo_image_surface_get_width (s) ||
     y < 0 || y >= cairo_image_surface_get_height (s))
        return 0;

    return * ((uint32_t *) (cairo_image_surface_get_data (s) +
     cairo_image_surface_get_stride (s) * y) + x) & 0xffffff;
}

void surface_copy_rect (cairo_surface_t * a, int ax, int ay, int w, int h,
 cairo_surface_t * b, int bx, int by)
{
    cairo_t * cr = cairo_create (b);
    cairo_set_source_surface (cr, a, bx - ax, by - ay);
    cairo_rectangle (cr, bx, by, w, h);
    cairo_fill (cr);
    cairo_destroy (cr);
}

void ServerSurface::clear ()
{
    if (m_surface)
        cairo_surface_destroy (m_surface);
    if (m_device)
        cairo_device_destroy (m_device);

    m_surface = nullptr;
    m_device = nullptr;
}

cairo_surface_t * ServerSurface::get (cairo_t * cr, cairo_surface_t * image,
 int scale, cairo_filter_t filter)
{
    if (! config.server_pixmaps)
        return nullptr;

    cairo_surface_t * target = cairo_get_group_target (cr);
    if (cairo_surface_get_type (target) != CAIRO_SURFACE_TYPE_XLIB)
        return nullptr;

    cairo_device_t * device = cairo_surface_get_device (target);
    double device_scale = 1, device_scale_y = 1;
    cairo_surface_get_device_scale (target, & device_scale, & device_scale_y);

    if (! device || device_scale != device_scale_y)
        return nullptr;

    if (m_surface && m_device == device && m_device_scale == device_scale &&
     m_scale == scale && cairo_surface_status (m_surface) == CAIRO_STATUS_SUCCESS)
        return m_surface;

    clear ();

    /* Without smoothing, the copy is a pixmap of the depth of the window (and
     * with its window scale), which is drawn from with XCopyArea. */
    bool blurred = (filter != CAIRO_FILTER_NEAREST && device_scale != 1);
    int border = blurred ? 1 : 0;

    cairo_surface_t * surface = cairo_surface_create_similar (target,
     blurred ? CAIRO_CONTENT_COLOR_ALPHA : CAIRO_CONTENT_COLOR,
     cairo_image_surface_get_width (image) * scale + 2 * border,
     cairo_image_surface_get_height (image) * scale + 2 * border);

    if (cairo_surface_status (surface) != CAIRO_STATUS_SUCCESS ||
     cairo_surface_get_type (surface) != CAIRO_SURFACE_TYPE_XLIB)
    {
        cairo_surface_destroy (surface);
        return nullptr;
    }

    cairo_t * copy = cairo_create (surface);
    cairo_translate (copy, border, border);
    cairo_scale (copy, scale, scale);
    cairo_set_source_surface (copy, image, 0, 0);
    cairo_pattern_set_filter (cairo_get_source (copy), filter);
    cairo_pattern_set_extend (cairo_get_source (copy),
     blurred ? CAIRO_EXTEND_NONE : CAIRO_EXTEND_PAD);
    cairo_set_operator (copy, CAIRO_OPERATOR_SOURCE);
    cairo_paint (copy);

    bool ok = (cairo_status (copy) == CAIRO_STATUS_SUCCESS);
    cairo_destroy (copy);

    if (! ok)
    {
        cairo_surface_destroy (surface);
        return nullptr;
    }

    AUDDBG ("%dx%d image copied to the X server (scale %d, window scale %g).\n",
     cairo_image_surface_get_width (image),
     cairo_image_surface_get_height (image), scale, device_scale);

    m_surface = surface;
    m_device = cairo_device_reference (device);
    m_device_scale = device_scale;
    m_scale = scale;
    m_border = border;

    return m_surface;
}

void surface_draw_rect (cairo_t * cr, cairo_surface_t * image,
 ServerSurface & copy, int xsrc, int ysrc, int xdest, int ydest, int width,
 int height)
{
    /* The copy in the server is scaled in advance by the scale factor of the
     * context, which the widgets set to a whole number (see Widget::draw_cb);
     * anything else is drawn from the image. */
    cairo_matrix_t m;
    cairo_get_matrix (cr, & m);

    int scale = (int) m.xx;
    cairo_surface_t * server = nullptr;

    if (scale >= 1 && m.xx == scale && m.yy == scale && m.xy == 0 && m.yx == 0)
        server = copy.get (cr, image, scale, CAIRO_FILTER_NEAREST);

    if (server)
    {
        cairo_save (cr);

        /* the same place, in pixels of the copy */
        cairo_matrix_init_translate (& m, m.x0, m.y0);
        cairo_set_matrix (cr, & m);

        cairo_set_source_surface (cr, server, (xdest - xsrc) * scale,
         (ydest - ysrc) * scale);
        cairo_rectangle (cr, xdest * scale, ydest * scale, width * scale,
         height * scale);
        cairo_fill (cr);

        cairo_restore (cr);
    }
    else
    {
        cairo_set_source_surface (cr, image, xdest - xsrc, ydest - ysrc);
        cairo_pattern_set_filter (cairo_get_source (cr), CAIRO_FILTER_NEAREST);
        cairo_rectangle (cr, xdest, ydest, width, height);
        cairo_fill (cr);
    }
}
