import os
import re
from pandas import DataFrame
import seaborn as sns
import matplotlib.pyplot as plt

MEASURED_FIELDS = ["Failures",
                   "Insert Success",
                   "Delete Success",
                   "Read Success",
                   "Scan Success",
                   "Update Success",
                   "Total Time"]

JUST_TIME = ["Total Time"]
data_dir = "./ycsb"


def parse_fields(resultlines):
    # The name thing is just to make sure that field names in the input
    # file and in the parsed data really match
    field_values = {field_name(name, line): values(line)
                    for (name, line) in zip(MEASURED_FIELDS, resultlines)}
    return field_values


def field_name(expected_name, line):
    re_field = re.compile("(?P<field>{}):".format(expected_name))
    return re.search(re_field, line).group("field")


def values(line):
    return [int(val) for val in re.findall(r'\b\d+\b', line)]


def parse_file(log_file_path):
    wl_name = wl_dir_name
    wl_name = wl_name.replace("m3-", "")
    wl_name = wl_name.replace("-results", "")
    wl_name = wl_name.replace("-original", "")
    version = "original" if "original" in wl_dir_name else "rewritten"
    measures = {"workload": wl_name, "version": version}
    with open(log_file_path, 'r') as log_file:
        lines = log_file.readlines()
        for (ln_num, ln) in enumerate(lines):
            if "Final Statistic" in ln:
                fst_measure = ln_num + 1
                field_lines = lines[fst_measure: fst_measure + len(MEASURED_FIELDS)]
                # I currently don't need the other fields but for now I'll keep the infrastructure for
                # parsing in case I can get more interesting things from m3
                parsed = parse_fields(field_lines)
                measures["time in ns"] = parsed["Total Time"]
    return measures



dicts = []
for wl_dir_name in os.listdir(data_dir):
    measures_dict = parse_file(os.path.join(data_dir, wl_dir_name, "log.txt"))
    dicts.append(measures_dict)


data = DataFrame(data=dicts)
data = data.explode("time in ns")
data["time in s"] = data["time in ns"]/1000_000_000

plot = sns.barplot(data=data, x="workload", y="time in s", hue="version")
plt.show()



# ToDo:
#       averaging and std should be left to the plot lib,
#       also, I'll need a way to merge results from the currently distinct branches original and rewritten
#       propably the best thing for now is a linewise write out of results as csv, since its easy to feed into seaborn
#       like wl_name  *time*, *time*, *time* ....
#            wl_name2 *time*, *time*, *time* ....
#       counts of operations don't seem to important at this point.




