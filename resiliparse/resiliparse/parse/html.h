/*
    Copyright 2021 Janek Bevendorff

    Licensed under the Apache License, Version 2.0 (the "License");
    you may not use this file except in compliance with the License.
    You may obtain a copy of the License at

        http://www.apache.org/licenses/LICENSE-2.0

    Unless required by applicable law or agreed to in writing, software
    distributed under the License is distributed on an "AS IS" BASIS,
    WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
    See the License for the specific language governing permissions and
    limitations under the License.
*/

#ifndef RESILIPARSE_PARSE_HTML_H
#define RESILIPARSE_PARSE_HTML_H

#include <lexbor/tag/const.h>

static const lxb_tag_id_t BLOCK_ELEMENTS[] = {
    LXB_TAG_ADDRESS,
    LXB_TAG_ARTICLE,
    LXB_TAG_ASIDE,
    LXB_TAG_BLOCKQUOTE,
    LXB_TAG_BR,
    LXB_TAG_CENTER,
    LXB_TAG_DETAILS,
    LXB_TAG_DD,
    LXB_TAG_DT,
    LXB_TAG_DIV,
    LXB_TAG_DL,
    LXB_TAG_FIELDSET,
    LXB_TAG_FIGCAPTION,
    LXB_TAG_FIGURE,
    LXB_TAG_FOOTER,
    LXB_TAG_FORM,
    LXB_TAG_H1,
    LXB_TAG_H2,
    LXB_TAG_H3,
    LXB_TAG_H4,
    LXB_TAG_H5,
    LXB_TAG_H6,
    LXB_TAG_HEADER,
    LXB_TAG_HGROUP,
    LXB_TAG_HR,
    LXB_TAG_LI,
    LXB_TAG_MAIN,
    LXB_TAG_NAV,
    LXB_TAG_OL,
    LXB_TAG_P,
    LXB_TAG_PRE,
    LXB_TAG_SECTION,
    LXB_TAG_TABLE,
    LXB_TAG_TR,
    LXB_TAG_UL
};
static const size_t NUM_BLOCK_ELEMENTS = sizeof(BLOCK_ELEMENTS) / sizeof(lxb_tag_id_t);

#endif  // RESILIPARSE_PARSE_HTML_H
