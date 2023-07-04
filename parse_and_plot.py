import os
import re

MEASURED_FIELDS = ["Failures",
                   "Insert Success",
                   "Delete Success",
                   "Read Success",
                   "Scan Success",
                   "Update Success",
                   "Total Time"]
output_dir = "./ycsb"


def parse_fields(resultlines):
    # The name thing is just to make sure that field names in the input
    # file and in the parsed data really match
    field_values = {field_name(name, line): values(line)
                    for (name, line) in zip(MEASURED_FIELDS, resultlines)}
    print(field_values)


def field_name(expected_name, line):
    re_field = re.compile("(?P<field>{}):".format(expected_name))
    return re.search(re_field, line).group("field")


def values(line):
    return [int(val) for val in re.findall(r'\b\d+\b', line)]


def parse_file(log_file_path):
    with open(log_file_path, 'r') as log_file:
        lines = log_file.readlines()
        for (ln_num, ln) in enumerate(lines):
            if "Final Statistic" in ln:
                fst_measure = ln_num + 1
                field_lines = lines[fst_measure: fst_measure + len(MEASURED_FIELDS)]
                return parse_fields(field_lines)
        print("No Statistics found in {}".format(log_file_path))
        return {}


for wl_dir_name in os.listdir(output_dir):
    wl_name = wl_dir_name
    wl_name = wl_name.replace("m3-", "")
    wl_name = wl_name.replace("-results", "")
    measures_dict = parse_file(os.path.join(output_dir, wl_dir_name, "log.txt"))
    measures_dict["workload"] = wl_name


# ToDo:
#       averaging and std should be left to the plot lib,
#       also, I'll need a way to merge results from the currently distinct brances original and rewritten
#       propably the best thing for now is a linwise writeout of results as csv, since its easy to feed into seaborn
#       like wl_name  *time*, *time*, *time* ....
#            wl_name2 *time*, *time*, *time* ....
#       counts of operations don't seem to important at this point.




